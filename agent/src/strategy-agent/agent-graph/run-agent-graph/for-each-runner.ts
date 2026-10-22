import { HumanMessage } from '@langchain/core/messages'
import type { Result } from 'neverthrow'
import { err, ok } from 'neverthrow'

import { isPlainObject } from '#strategy-agent/agent-graph/json'
import { createPreviousStepMatcher } from '#strategy-agent/agent-graph/resume'
import {
  getExecutionAbortMessage,
  isExecutionAborted,
} from '#strategy-agent/agent-graph/run-agent-graph/execution-abort'
import { buildPhaseMessageText } from '#strategy-agent/agent-graph/run-agent-graph/phase-message'
import {
  errorMessage,
  invokeAndRecordStep,
} from '#strategy-agent/agent-graph/run-agent-graph/step-execution'
import type {
  RunAgentGraphContext,
  RunAgentGraphDeps,
  StepRecorder,
} from '#strategy-agent/agent-graph/run-agent-graph/types'
import type { StrategyTaskStep } from '#strategy-agent/agent-graph/step'
import type { AgentGraphPhase } from '#strategy-agent/agent-graph/types'
import { isUsageLimitError } from '#strategy-agent/usage-limit'

const extractItemLabel = (
  item: unknown,
  labelField: string | undefined,
): string | undefined => {
  if (labelField === undefined || !isPlainObject(item)) return undefined
  const value = item[labelField]
  return typeof value === 'string' ? value : undefined
}

type ForEachItemOutcome =
  | { readonly kind: 'success'; readonly value: unknown }
  | { readonly kind: 'error'; readonly error: unknown }

// あえてセマンティックな語彙は含めない: モデルに渡すのは元のリクエスト、
// このフェーズの指示、現在の for_each 対象、手前のフェーズの出力という
// 汎用的なパイプライン構造のみ。
// for_each の形式 ("<key>.<field>") は backend が PUT 時にバリデーション
// 済みのため、ここでは検証を伴わない単純な最初の "." での分割で済ませる。
export const splitForEach = (forEach: string): readonly [string, string] => {
  const dotIndex = forEach.indexOf('.')
  return [forEach.slice(0, dotIndex), forEach.slice(dotIndex + 1)]
}

// フェーズ key -> 後続フェーズの for_each が配列を要求する output フィールド名の集合。
// 欠落または非配列の値は、構造化出力のスキーマ検証と同じ再試行ループで再試行する。
export const collectRequiredArrayFields = (
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

// 参照元フェーズ (invokePhaseWithRetry) が配列を返すまで再試行済み。
// 参照先フェーズが存在しない場合などの防御的フォールバックとして再確認する。
export const resolveForEachItems = (
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
  return ok(items)
}

export const runForEachItems = async (
  deps: RunAgentGraphDeps,
  phase: AgentGraphPhase,
  context: RunAgentGraphContext,
  priorResults: Readonly<Record<string, unknown>>,
  phaseKeys: readonly string[],
  items: readonly unknown[],
  recorder: StepRecorder,
  requiredArrayFields: ReadonlySet<string>,
  previousStepsForPhase: readonly StrategyTaskStep[],
  reuseCompleted: boolean,
): Promise<Result<unknown[], unknown>> => {
  const concurrency = Math.max(phase.maxParallel ?? items.length, 1)
  const workerCount = Math.min(concurrency, items.length)
  const results: (ForEachItemOutcome | undefined)[] = []
  let nextIndex = 0
  const outputs: unknown[] = []
  let firstError: unknown
  const previousStepMatcher = createPreviousStepMatcher(previousStepsForPhase)

  if (isExecutionAborted(context)) {
    return err(new Error(getExecutionAbortMessage(context)))
  }

  const runItem = (index: number): Promise<Result<unknown, unknown>> => {
    const item = items[index]
    const itemLabel = extractItemLabel(item, phase.labelField)
    const matched = previousStepMatcher.take(item)
    if (reuseCompleted && matched?.status === 'completed') {
      recorder.recordExisting(matched)
      return Promise.resolve(ok(matched.output))
    }
    const messageText = buildPhaseMessageText({
      originalPromptText: context.originalPromptText,
      phaseKeys,
      phaseKey: phase.key,
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
  }

  const runWorker = async (): Promise<boolean> => {
    let isFirstItem = true
    while (nextIndex < items.length) {
      if (!isFirstItem && isExecutionAborted(context)) {
        return true
      }
      const index = nextIndex
      nextIndex++
      isFirstItem = false
      const itemResult = await runItem(index)
      results[index] = itemResult.match(
        (value): ForEachItemOutcome => ({ kind: 'success', value }),
        (error): ForEachItemOutcome => ({ kind: 'error', error }),
      )
    }
    return false
  }

  const workerDeadlines = await Promise.all(
    Array.from({ length: workerCount }, () => runWorker()),
  )

  if (workerDeadlines.includes(true)) {
    return err(new Error(getExecutionAbortMessage(context)))
  }

  for (const itemResult of results) {
    if (itemResult === undefined) continue
    if (itemResult.kind === 'error') {
      firstError ??= itemResult.error
    } else {
      outputs.push(itemResult.value)
    }
  }

  if (items.length > 0 && outputs.length === 0) {
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
