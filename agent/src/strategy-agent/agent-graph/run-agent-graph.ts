import { HumanMessage } from '@langchain/core/messages'
import type { Result } from 'neverthrow'
import { err, ok } from 'neverthrow'

import { isPlainObject } from '#strategy-agent/agent-graph/json'
import {
  createPreviousStepMatcher,
  findPreviousStepForPhase,
} from '#strategy-agent/agent-graph/resume'
import {
  createStepRecorder,
  errorMessage,
  invokeAndRecordStep,
} from '#strategy-agent/agent-graph/run-agent-graph/step-execution'
import type {
  RunAgentGraphContext,
  RunAgentGraphDeps,
  StepRecorder,
} from '#strategy-agent/agent-graph/run-agent-graph/types'
import type { StrategyTaskStep } from '#strategy-agent/agent-graph/step'
import type {
  AgentGraphConfig,
  AgentGraphPhase,
} from '#strategy-agent/agent-graph/types'
import type { StrategyAgentResult } from '#strategy-agent/strategy-agent'
import { isUsageLimitError } from '#strategy-agent/usage-limit'

export type {
  BuildPhaseAgentOptions,
  CompiledPhaseAgent,
  RunAgentGraphContext,
  RunAgentGraphDeps,
} from '#strategy-agent/agent-graph/run-agent-graph/types'

const buildFailureResult = (
  phase: AgentGraphPhase,
  error: unknown,
): StrategyAgentResult => ({
  status: 'failed',
  message: `フェーズ「${phase.label}」(${phase.key}) の実行に失敗しました: ${errorMessage(error)}`,
  errorKind: isUsageLimitError(error) ? 'usage_limit' : 'agent_error',
})

const extractItemLabel = (
  item: unknown,
  labelField: string | undefined,
): string | undefined => {
  if (labelField === undefined || !isPlainObject(item)) return undefined
  const value = item[labelField]
  return typeof value === 'string' ? value : undefined
}

// あえてセマンティックな語彙は含めない: モデルに渡すのは元のリクエスト、
// このフェーズの指示、現在の for_each 対象、手前のフェーズの出力という
// 汎用的なパイプライン構造のみ。
export const buildPhaseMessageText = (input: {
  readonly originalPromptText: string
  readonly phasePrompt: string
  readonly item: unknown
  readonly priorResults: Readonly<Record<string, unknown>>
}): string => {
  const sections = [input.originalPromptText, input.phasePrompt]
  if (input.item !== undefined) {
    sections.push(
      `割り当てられた対象:\n\`\`\`json\n${JSON.stringify(input.item, null, 2)}\n\`\`\``,
    )
  }
  if (Object.keys(input.priorResults).length > 0) {
    sections.push(
      `これまでのフェーズの結果:\n\`\`\`json\n${JSON.stringify(input.priorResults, null, 2)}\n\`\`\``,
    )
  }
  return sections.join('\n\n---\n\n')
}

// for_each の形式 ("<key>.<field>") は backend が PUT 時にバリデーション
// 済みのため、ここでは検証を伴わない単純な最初の "." での分割で済ませる。
const splitForEach = (forEach: string): readonly [string, string] => {
  const dotIndex = forEach.indexOf('.')
  return [forEach.slice(0, dotIndex), forEach.slice(dotIndex + 1)]
}

// フェーズ key -> 後続フェーズの for_each から非空配列であることを要求されて
// いる自分の output フィールド名の集合。この集合が空でないフィールドは、
// 構造化出力のスキーマ検証と同じ再試行ループで「非空配列を返すまで」再試行
// する対象になる (for_each の参照先が空/欠落のまま先に進むのを防ぐため)。
const collectRequiredArrayFields = (
  phases: readonly AgentGraphPhase[],
): ReadonlyMap<string, ReadonlySet<string>> => {
  const map = new Map<string, Set<string>>()
  for (const phase of phases) {
    if (phase.forEach === undefined) continue
    const [refKey, refField] = splitForEach(phase.forEach)
    const fields = map.get(refKey) ?? new Set<string>()
    fields.add(refField)
    map.set(refKey, fields)
  }
  return map
}

// 参照元フェーズ (invokePhaseWithRetry) が非空配列を返すまで再試行済みのため、
// ここに到達した時点で参照先は本来常に妥当な非空配列のはず。このチェックは
// その前提が崩れた場合 (参照先フェーズが存在しない等) の防御的フォールバック。
const resolveForEachItems = (
  priorResults: Readonly<Record<string, unknown>>,
  refKey: string,
  refField: string,
): Result<readonly unknown[], Error> => {
  const referenced = priorResults[refKey]
  const items = isPlainObject(referenced) ? referenced[refField] : undefined
  if (!Array.isArray(items)) {
    return err(
      new Error(
        `for_each の参照先 "${refKey}.${refField}" が配列ではありません`,
      ),
    )
  }
  if (items.length === 0) {
    return err(
      new Error(`for_each の参照先 "${refKey}.${refField}" が空配列です`),
    )
  }
  return ok(items)
}

const runForEachItems = async (
  deps: RunAgentGraphDeps,
  phase: AgentGraphPhase,
  context: RunAgentGraphContext,
  priorResults: Readonly<Record<string, unknown>>,
  items: readonly unknown[],
  recorder: StepRecorder,
  requiredArrayFields: ReadonlySet<string>,
  previousStepsForPhase: readonly StrategyTaskStep[],
): Promise<Result<unknown[], unknown>> => {
  // 固定サイズのチャンク分割による並列数制御。セマフォより単純だが、フェーズあたりの
  // レイテンシ差が大きい場合は待ち時間が偏る。偏りが問題になれば worker pool 方式に置き換える。
  const chunkSize = Math.max(phase.maxParallel ?? items.length, 1)
  const outputs: unknown[] = []
  let firstError: unknown
  const previousStepMatcher = createPreviousStepMatcher(previousStepsForPhase)

  for (let start = 0; start < items.length; start += chunkSize) {
    const chunk = items.slice(start, start + chunkSize)
    const chunkResults = await Promise.all(
      chunk.map((item, offset) => {
        const index = start + offset
        const itemLabel = extractItemLabel(item, phase.labelField)
        const matched = previousStepMatcher.take(item)
        if (matched?.status === 'completed') {
          recorder.recordExisting(matched)
          return Promise.resolve(ok(matched.output))
        }
        const messageText = buildPhaseMessageText({
          originalPromptText: context.originalPromptText,
          phasePrompt: phase.prompt,
          item,
          priorResults,
        })
        return invokeAndRecordStep(
          deps,
          phase,
          context,
          [new HumanMessage(messageText)],
          requiredArrayFields,
          recorder,
          {
            phaseKey: phase.key,
            label: phase.label,
            model: phase.model,
            item,
            ...(itemLabel !== undefined ? { itemLabel } : {}),
          },
          `${phase.label} (${String(index + 1)}/${String(items.length)})`,
          {
            'phase.key': phase.key,
            'phase.model': phase.model,
            'phase.item_index': index,
          },
          matched?.executionStepId,
        )
      }),
    )
    for (const chunkResult of chunkResults) {
      if (chunkResult.isErr()) {
        firstError ??= chunkResult.error
      } else {
        outputs.push(chunkResult.value)
      }
    }
  }

  if (outputs.length === 0) {
    return err(
      isUsageLimitError(firstError)
        ? firstError
        : new Error(
            `for_each の全要素 (${String(items.length)}件) が失敗しました: ${errorMessage(firstError)}`,
          ),
    )
  }

  return ok(outputs)
}

const runPhase = async (
  deps: RunAgentGraphDeps,
  phase: AgentGraphPhase,
  context: RunAgentGraphContext,
  priorResults: Readonly<Record<string, unknown>>,
  recorder: StepRecorder,
  requiredArrayFieldsByPhase: ReadonlyMap<string, ReadonlySet<string>>,
): Promise<Result<unknown, unknown>> => {
  const requiredArrayFields =
    requiredArrayFieldsByPhase.get(phase.key) ?? new Set<string>()

  if (phase.forEach === undefined) {
    const previous = findPreviousStepForPhase(context.previousSteps, phase.key)
    if (previous?.status === 'completed') {
      recorder.recordExisting(previous)
      return ok(previous.output)
    }

    const messageText = buildPhaseMessageText({
      originalPromptText: context.originalPromptText,
      phasePrompt: phase.prompt,
      item: undefined,
      priorResults,
    })
    return invokeAndRecordStep(
      deps,
      phase,
      context,
      [new HumanMessage(messageText)],
      requiredArrayFields,
      recorder,
      { phaseKey: phase.key, label: phase.label, model: phase.model },
      phase.label,
      { 'phase.key': phase.key, 'phase.model': phase.model },
      previous?.executionStepId,
    )
  }

  const [refKey, refField] = splitForEach(phase.forEach)
  const itemsResult = resolveForEachItems(priorResults, refKey, refField)
  if (itemsResult.isErr()) return itemsResult

  const previousStepsForPhase =
    context.previousSteps?.filter((s) => s.phaseKey === phase.key) ?? []

  return runForEachItems(
    deps,
    phase,
    context,
    priorResults,
    itemsResult.value,
    recorder,
    requiredArrayFields,
    previousStepsForPhase,
  )
}

export const runAgentGraph = async (
  deps: RunAgentGraphDeps,
  config: AgentGraphConfig,
  context: RunAgentGraphContext,
): Promise<StrategyAgentResult> => {
  const results: Record<string, unknown> = {}
  const recorder = createStepRecorder(context.onStepsChanged)
  const requiredArrayFieldsByPhase = collectRequiredArrayFields(config.phases)

  for (const phase of config.phases) {
    const phaseResult = await runPhase(
      deps,
      phase,
      context,
      results,
      recorder,
      requiredArrayFieldsByPhase,
    )
    if (phaseResult.isErr()) {
      return buildFailureResult(phase, phaseResult.error)
    }
    results[phase.key] = phaseResult.value
  }

  return {
    status: 'completed',
    message: `${String(config.phases.length)}フェーズの実行が完了しました (${config.phases.map((p) => p.label).join(' → ')})`,
  }
}
