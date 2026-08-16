import type { BaseChatModel } from '@langchain/core/language_models/chat_models'
import { HumanMessage } from '@langchain/core/messages'
import type { DynamicStructuredTool } from '@langchain/core/tools'
import type { Result } from 'neverthrow'
import { err, ok } from 'neverthrow'

import { isPlainObject } from '#strategy-agent/agent-graph/json'
import type { ObjectJsonSchema } from '#strategy-agent/agent-graph/output-schema'
import { buildOutputJsonSchema } from '#strategy-agent/agent-graph/output-schema'
import type { StrategyTaskStep } from '#strategy-agent/agent-graph/step'
import { withPhaseSpan } from '#strategy-agent/agent-graph/tracing'
import type {
  AgentGraphConfig,
  AgentGraphPhase,
} from '#strategy-agent/agent-graph/types'
import type { StrategyAgentResult } from '#strategy-agent/strategy-agent'
import { buildSystemPrompt } from '#strategy-agent/system-prompt'
import { isUsageLimitError } from '#strategy-agent/usage-limit'

// 上限に達しても structured response を得られなければ、そのフェーズを失敗として確定する。
const MAX_STRUCTURED_OUTPUT_ATTEMPTS = 3

export interface BuildPhaseAgentOptions {
  readonly model: BaseChatModel
  readonly tools: readonly DynamicStructuredTool[]
  readonly systemPrompt: string
  readonly responseSchema: ObjectJsonSchema
}

export interface CompiledPhaseAgent {
  invoke(input: { messages: readonly HumanMessage[] }): Promise<{
    structuredResponse?: Record<string, unknown>
  }>
}

export interface RunAgentGraphDeps {
  readonly buildPhaseAgent: (
    options: BuildPhaseAgentOptions,
  ) => CompiledPhaseAgent
  readonly createChatModel: (model: string) => BaseChatModel
}

export interface RunAgentGraphContext {
  readonly agentsMd: string
  readonly skills: Readonly<Record<string, string>>
  readonly tools: readonly DynamicStructuredTool[]
  readonly originalPromptText: string
  // 実行中のフェーズ/for_each 要素ごとの進捗を都度通知する。呼び出し側は
  // 受け取った配列全体を steps の最新状態として扱う (差分ではない)。
  readonly onStepsChanged?: (steps: readonly StrategyTaskStep[]) => void
}

const errorMessage = (error: unknown): string =>
  error instanceof Error ? error.message : String(error)

const buildFailureResult = (
  phase: AgentGraphPhase,
  error: unknown,
): StrategyAgentResult => ({
  status: 'failed',
  message: `フェーズ「${phase.label}」(${phase.key}) の実行に失敗しました: ${errorMessage(error)}`,
  errorKind: isUsageLimitError(error) ? 'usage_limit' : 'agent_error',
})

type StepStartInput = Omit<
  StrategyTaskStep,
  'status' | 'finishedAt' | 'output' | 'error'
>
type StepOutcome =
  | { readonly status: 'completed'; readonly output: unknown }
  | { readonly status: 'failed'; readonly error: string }

interface StepRecorder {
  readonly start: (step: StepStartInput) => number
  readonly finish: (index: number, outcome: StepOutcome) => void
}

// steps 配列はここでのみ mutate する。呼び出し側 (executor) は
// onStepsChanged で渡された配列を都度「最新の全体」として扱えばよい。
const createStepRecorder = (
  onStepsChanged: RunAgentGraphContext['onStepsChanged'],
): StepRecorder => {
  const steps: StrategyTaskStep[] = []
  const notify = (): void => onStepsChanged?.(steps.slice())

  return {
    start: (step) => {
      steps.push({ ...step, status: 'running' })
      notify()
      return steps.length - 1
    },
    finish: (index, outcome) => {
      const current = steps[index]
      if (current === undefined) return
      steps[index] = {
        ...current,
        ...outcome,
        finishedAt: new Date().toISOString(),
      }
      notify()
    },
  }
}

const extractItemLabel = (
  item: unknown,
  labelField: string | undefined,
): string | undefined => {
  if (labelField === undefined || !isPlainObject(item)) return undefined
  const value = item[labelField]
  return typeof value === 'string' ? value : undefined
}

const createPhaseAgent = (
  deps: RunAgentGraphDeps,
  phase: AgentGraphPhase,
  context: RunAgentGraphContext,
): CompiledPhaseAgent => {
  // 省略時は全 tool を許可する (単一フェーズの現行挙動と同じ)。
  const { tools: phaseTools } = phase
  const filteredTools =
    phaseTools === undefined
      ? context.tools
      : context.tools.filter((tool) => phaseTools.includes(tool.name))
  const filteredSkills: Record<string, string> = {}
  for (const name of phase.skills) {
    const body = context.skills[name]
    if (body !== undefined) filteredSkills[name] = body
  }
  return deps.buildPhaseAgent({
    model: deps.createChatModel(phase.model),
    tools: filteredTools,
    systemPrompt: buildSystemPrompt({
      agentsMd: context.agentsMd,
      skills: filteredSkills,
    }),
    responseSchema: buildOutputJsonSchema(phase.output),
  })
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

const hasRequiredArrayFields = (
  response: Record<string, unknown>,
  requiredArrayFields: ReadonlySet<string>,
): boolean =>
  [...requiredArrayFields].every(
    (field) => Array.isArray(response[field]) && response[field].length > 0,
  )

// invoke() 自体の reject (usage limit・ツール失敗・ネットワークエラー等) は
// 再試行せず即座に伝播する。再試行するのは invoke が成功したにもかかわらず
// structured response を欠く場合と、structured response はあるが
// requiredArrayFields (後続フェーズの for_each が要求する非空配列) を
// 満たさない場合のみで、これが再試行で解消しうる唯一の失敗モードのため。
const invokePhaseWithRetry = async (
  agent: CompiledPhaseAgent,
  messages: readonly HumanMessage[],
  requiredArrayFields: ReadonlySet<string>,
  attemptsLeft: number = MAX_STRUCTURED_OUTPUT_ATTEMPTS,
): Promise<Result<Record<string, unknown>, unknown>> => {
  const invoked = await agent.invoke({ messages }).then(
    (
      value,
    ): Result<{ structuredResponse?: Record<string, unknown> }, unknown> =>
      ok(value),
    (
      error: unknown,
    ): Result<{ structuredResponse?: Record<string, unknown> }, unknown> =>
      err(error),
  )
  if (invoked.isErr()) return invoked
  const { structuredResponse } = invoked.value
  if (
    structuredResponse !== undefined &&
    hasRequiredArrayFields(structuredResponse, requiredArrayFields)
  ) {
    return ok(structuredResponse)
  }
  if (attemptsLeft <= 1) {
    return err(
      structuredResponse === undefined
        ? new Error('agent did not return a structured response')
        : new Error(
            `agent's structured response did not resolve required for_each field(s) to a non-empty array: ${[...requiredArrayFields].join(', ')}`,
          ),
    )
  }
  return invokePhaseWithRetry(
    agent,
    messages,
    requiredArrayFields,
    attemptsLeft - 1,
  )
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

// 1 件分の invoke を実行し、開始時に running step を記録、決着したら
// completed/failed に更新する。for_each の各要素と、for_each でないフェーズ
// (常に 1 件) の両方から呼ばれる。
const invokeAndRecordStep = (
  agent: CompiledPhaseAgent,
  messages: readonly HumanMessage[],
  requiredArrayFields: ReadonlySet<string>,
  recorder: StepRecorder,
  stepBase: Omit<StepStartInput, 'startedAt' | 'traceId' | 'spanId'>,
  spanName: string,
  spanAttributes: Record<string, string | number>,
): Promise<Result<Record<string, unknown>, unknown>> =>
  withPhaseSpan(spanName, spanAttributes, (spanIds) => {
    const index = recorder.start({
      ...stepBase,
      startedAt: new Date().toISOString(),
      traceId: spanIds.traceId,
      spanId: spanIds.spanId,
    })
    return invokePhaseWithRetry(agent, messages, requiredArrayFields).then(
      (result) => {
        if (result.isErr()) {
          recorder.finish(index, {
            status: 'failed',
            error: errorMessage(result.error),
          })
        } else {
          recorder.finish(index, {
            status: 'completed',
            output: result.value,
          })
        }
        return result
      },
    )
  })

const runForEachItems = async (
  phase: AgentGraphPhase,
  context: RunAgentGraphContext,
  priorResults: Readonly<Record<string, unknown>>,
  agent: CompiledPhaseAgent,
  items: readonly unknown[],
  recorder: StepRecorder,
  requiredArrayFields: ReadonlySet<string>,
): Promise<Result<unknown[], unknown>> => {
  // 固定サイズのチャンク分割による並列数制御。セマフォより単純だが、フェーズあたりの
  // レイテンシ差が大きい場合は待ち時間が偏る。偏りが問題になれば worker pool 方式に置き換える。
  const chunkSize = Math.max(phase.maxParallel ?? items.length, 1)
  const outputs: unknown[] = []

  for (let start = 0; start < items.length; start += chunkSize) {
    const chunk = items.slice(start, start + chunkSize)
    const chunkResults = await Promise.all(
      chunk.map((item, offset) => {
        const index = start + offset
        const itemLabel = extractItemLabel(item, phase.labelField)
        const messageText = buildPhaseMessageText({
          originalPromptText: context.originalPromptText,
          phasePrompt: phase.prompt,
          item,
          priorResults,
        })
        return invokeAndRecordStep(
          agent,
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
        )
      }),
    )
    for (const chunkResult of chunkResults) {
      if (chunkResult.isErr()) return err(chunkResult.error)
      outputs.push(chunkResult.value)
    }
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
  const agent = createPhaseAgent(deps, phase, context)
  const requiredArrayFields =
    requiredArrayFieldsByPhase.get(phase.key) ?? new Set<string>()

  if (phase.forEach === undefined) {
    const messageText = buildPhaseMessageText({
      originalPromptText: context.originalPromptText,
      phasePrompt: phase.prompt,
      item: undefined,
      priorResults,
    })
    return invokeAndRecordStep(
      agent,
      [new HumanMessage(messageText)],
      requiredArrayFields,
      recorder,
      { phaseKey: phase.key, label: phase.label, model: phase.model },
      phase.label,
      { 'phase.key': phase.key, 'phase.model': phase.model },
    )
  }

  const [refKey, refField] = splitForEach(phase.forEach)
  const itemsResult = resolveForEachItems(priorResults, refKey, refField)
  if (itemsResult.isErr()) return itemsResult

  return runForEachItems(
    phase,
    context,
    priorResults,
    agent,
    itemsResult.value,
    recorder,
    requiredArrayFields,
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
