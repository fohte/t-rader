import type { BaseChatModel } from '@langchain/core/language_models/chat_models'
import { HumanMessage } from '@langchain/core/messages'
import type { DynamicStructuredTool } from '@langchain/core/tools'
import type { Result } from 'neverthrow'
import { err, ok } from 'neverthrow'

import { isPlainObject } from '#strategy-agent/agent-graph/json'
import type { ObjectJsonSchema } from '#strategy-agent/agent-graph/output-schema'
import { buildOutputJsonSchema } from '#strategy-agent/agent-graph/output-schema'
import { withPhaseSpan } from '#strategy-agent/agent-graph/tracing'
import type {
  AgentGraphConfig,
  AgentGraphPhase,
} from '#strategy-agent/agent-graph/types'
import type { StrategyAgentResult } from '#strategy-agent/strategy-agent'
import { buildSystemPrompt } from '#strategy-agent/system-prompt'
import { isUsageLimitError } from '#strategy-agent/usage-limit'

// 初回 + 2 回まで再試行、という製品判断 (最大3回試みて駄目ならフェーズを失敗として確定させる)。
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

const createPhaseAgent = (
  deps: RunAgentGraphDeps,
  phase: AgentGraphPhase,
  context: RunAgentGraphContext,
): CompiledPhaseAgent => {
  const filteredTools = context.tools.filter((tool) =>
    phase.tools.includes(tool.name),
  )
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

// No semantic vocabulary here on purpose: only the generic pipeline shape
// (original request, this phase's instructions, the current for_each item,
// and prior phases' outputs) is surfaced to the model.
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

const invokePhaseWithRetry = async (
  agent: CompiledPhaseAgent,
  messages: readonly HumanMessage[],
  attemptsLeft: number = MAX_STRUCTURED_OUTPUT_ATTEMPTS,
): Promise<Result<Record<string, unknown>, unknown>> => {
  const invokeResult = await agent.invoke({ messages }).then(
    (value): Result<Record<string, unknown>, unknown> =>
      value.structuredResponse === undefined
        ? err(new Error('agent did not return a structured response'))
        : ok(value.structuredResponse),
    (error: unknown): Result<Record<string, unknown>, unknown> => err(error),
  )
  if (invokeResult.isOk() || attemptsLeft <= 1) return invokeResult
  return invokePhaseWithRetry(agent, messages, attemptsLeft - 1)
}

// backend validates for_each's format at PUT time (must be "<key>.<field>"),
// so this is a plain first-dot split rather than a validating parse.
const splitForEach = (forEach: string): readonly [string, string] => {
  const dotIndex = forEach.indexOf('.')
  return [forEach.slice(0, dotIndex), forEach.slice(dotIndex + 1)]
}

const resolveForEachItems = (
  priorResults: Readonly<Record<string, unknown>>,
  refKey: string,
  refField: string,
): Result<readonly unknown[], Error> => {
  const referenced = priorResults[refKey]
  const items = isPlainObject(referenced) ? referenced[refField] : undefined
  return Array.isArray(items)
    ? ok(items)
    : err(
        new Error(
          `for_each の参照先 "${refKey}.${refField}" が配列ではありません`,
        ),
      )
}

const runForEachItems = async (
  phase: AgentGraphPhase,
  context: RunAgentGraphContext,
  priorResults: Readonly<Record<string, unknown>>,
  agent: CompiledPhaseAgent,
  items: readonly unknown[],
): Promise<Result<unknown[], unknown>> => {
  // ponytail: 固定サイズのチャンク分割による並列数制御。セマフォより単純で、
  // フェーズあたりのレイテンシ差が大きくなるなら worker pool 方式に上げる。
  const chunkSize = Math.max(phase.maxParallel ?? items.length, 1)
  const outputs: unknown[] = []

  for (let start = 0; start < items.length; start += chunkSize) {
    const chunk = items.slice(start, start + chunkSize)
    const chunkResults = await Promise.all(
      chunk.map((item, offset) => {
        const index = start + offset
        const messageText = buildPhaseMessageText({
          originalPromptText: context.originalPromptText,
          phasePrompt: phase.prompt,
          item,
          priorResults,
        })
        return withPhaseSpan(
          `${phase.label} (${String(index + 1)}/${String(items.length)})`,
          {
            'phase.key': phase.key,
            'phase.model': phase.model,
            'phase.item_index': index,
          },
          () => invokePhaseWithRetry(agent, [new HumanMessage(messageText)]),
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
): Promise<Result<unknown, unknown>> => {
  const agent = createPhaseAgent(deps, phase, context)

  if (phase.forEach === undefined) {
    const messageText = buildPhaseMessageText({
      originalPromptText: context.originalPromptText,
      phasePrompt: phase.prompt,
      item: undefined,
      priorResults,
    })
    return withPhaseSpan(
      phase.label,
      { 'phase.key': phase.key, 'phase.model': phase.model },
      () => invokePhaseWithRetry(agent, [new HumanMessage(messageText)]),
    )
  }

  const [refKey, refField] = splitForEach(phase.forEach)
  const itemsResult = resolveForEachItems(priorResults, refKey, refField)
  if (itemsResult.isErr()) return itemsResult

  return runForEachItems(phase, context, priorResults, agent, itemsResult.value)
}

export const runAgentGraph = async (
  deps: RunAgentGraphDeps,
  config: AgentGraphConfig,
  context: RunAgentGraphContext,
): Promise<StrategyAgentResult> => {
  const results: Record<string, unknown> = {}

  for (const phase of config.phases) {
    const phaseResult = await runPhase(deps, phase, context, results)
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
