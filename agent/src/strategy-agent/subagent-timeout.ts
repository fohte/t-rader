import {
  AIMessage,
  type BaseMessage,
  HumanMessage,
  ToolMessage,
} from '@langchain/core/messages'
import { DynamicStructuredTool } from '@langchain/core/tools'
import { err, ok, type Result } from 'neverthrow'

import { buildOutputJsonSchema } from '#strategy-agent/agent-graph/output-schema'
import type {
  CompiledPhaseAgent,
  RunAgentGraphDeps,
  StepRecorder,
} from '#strategy-agent/agent-graph/run-agent-graph/types'
import type { AgentGraphPhase } from '#strategy-agent/agent-graph/types'
import type { ModelCallBudget } from '#strategy-agent/final-turn-middleware'
import {
  isPhaseTimeoutSignal,
  type PhaseTimeout,
  phaseTimeoutMessage,
} from '#strategy-agent/phase-timeout'
import type { McpToolsClient } from '#strategy-agent/strategy-agent'
import type {
  FetchSubagentDefinition,
  SubagentDefinition,
} from '#strategy-agent/subagent-definition-client'

export interface SubagentRunCounter {
  count: number
}

export interface CreateRunSubagentToolOptions {
  readonly phase: AgentGraphPhase
  readonly parentExecutionStepId: string
  readonly recorder: StepRecorder
  readonly runCounter: SubagentRunCounter
  readonly fetchDefinition: FetchSubagentDefinition
  readonly createMcpClient: (
    executionStepId: string,
    definitionKey: string,
  ) => McpToolsClient
  readonly deps: RunAgentGraphDeps
  readonly deadlineSignal?: AbortSignal
  readonly phaseTimeout?: PhaseTimeout
}

export const toError = (error: unknown): Error =>
  error instanceof Error ? error : new Error(String(error))

const getStructuredResponse = (
  agent: CompiledPhaseAgent,
  messages: readonly BaseMessage[],
  executionStepId: string,
): Promise<Record<string, unknown>> =>
  agent.invoke({ messages, executionStepId }).then((result) => {
    if (result.structuredResponse === undefined) {
      return Promise.reject(
        new Error('subagent returned no structured response'),
      )
    }
    return result.structuredResponse
  })

export const buildSubagentAgent = (
  definition: SubagentDefinition,
  deps: RunAgentGraphDeps,
  tools: readonly DynamicStructuredTool[],
  options: {
    readonly deadlineSignal?: AbortSignal
    readonly phaseTimeout?: PhaseTimeout
    readonly modelCallBudget: ModelCallBudget
    readonly systemPrompt?: string
  },
): CompiledPhaseAgent =>
  deps.buildPhaseAgent({
    model: deps.createChatModel(definition.model, {
      ...(definition.reasoningEffort !== undefined
        ? { reasoningEffort: definition.reasoningEffort }
        : {}),
    }),
    tools,
    systemPrompt: options.systemPrompt ?? definition.prompt,
    responseSchema: buildOutputJsonSchema(definition.output),
    modelCallBudget: options.modelCallBudget,
    captureMessagesOnAbort: true,
    ...(options.deadlineSignal !== undefined
      ? { deadlineSignal: options.deadlineSignal }
      : {}),
    ...(options.phaseTimeout !== undefined
      ? { phaseTimeout: options.phaseTimeout }
      : {}),
  })

export const getParentDeadlineError = (
  options: CreateRunSubagentToolOptions,
): Error => {
  const message = isPhaseTimeoutSignal(
    options.deadlineSignal,
    options.phaseTimeout,
  )
    ? phaseTimeoutMessage(options.phaseTimeout)
    : 'parent execution deadline exceeded'
  return new Error(message)
}

export const finalizeTimedOutSubagent = (
  definition: SubagentDefinition,
  executionStepId: string,
  options: CreateRunSubagentToolOptions,
  modelCallBudget: ModelCallBudget,
  messages: readonly BaseMessage[],
): Promise<Result<Record<string, unknown>, Error>> => {
  if (options.deadlineSignal?.aborted === true) {
    return Promise.resolve(err(getParentDeadlineError(options)))
  }
  if (modelCallBudget.count >= modelCallBudget.maxModelCalls) {
    return Promise.resolve(
      err(
        new Error(
          `subagent ${definition.key} reached its time limit after using all model calls`,
        ),
      ),
    )
  }
  // 調査は止め、親 deadline と子の呼び出し予算を共有したまま結果だけをまとめる。
  const finalizer = buildSubagentAgent(definition, options.deps, [], {
    ...(options.deadlineSignal !== undefined
      ? { deadlineSignal: options.deadlineSignal }
      : {}),
    ...(options.phaseTimeout !== undefined
      ? { phaseTimeout: options.phaseTimeout }
      : {}),
    modelCallBudget,
    systemPrompt: [
      definition.prompt,
      '制限時間に達しました。これまでに得た情報だけを使って結果を返してください。',
    ].join('\n\n'),
  })
  const completedToolCalls = completeInterruptedToolCalls(messages)
  return getStructuredResponse(
    finalizer,
    [
      ...completedToolCalls,
      new HumanMessage(
        '制限時間に達しました。追加調査は行わず、ここまでの情報から指定形式の結果を返してください。',
      ),
    ],
    executionStepId,
  ).then(
    (response) => ok(response),
    (error: unknown) => err(toError(error)),
  )
}

const completeInterruptedToolCalls = (
  messages: readonly BaseMessage[],
): BaseMessage[] => {
  const pendingToolCalls = new Map<string, string>()
  const completedMessages: BaseMessage[] = []
  for (const message of messages) {
    if (message instanceof AIMessage) {
      const toolCalls = message.tool_calls ?? []
      if (toolCalls.some((toolCall) => toolCall.id === undefined)) {
        break
      }
      for (const toolCall of toolCalls) {
        if (toolCall.id === undefined) continue
        pendingToolCalls.set(toolCall.id, toolCall.name)
      }
    } else if (message instanceof ToolMessage) {
      pendingToolCalls.delete(message.tool_call_id)
    }
    completedMessages.push(message)
  }
  if (pendingToolCalls.size === 0) return completedMessages
  return [
    ...completedMessages,
    ...[...pendingToolCalls].map(
      ([toolCallId, name]) =>
        new ToolMessage({
          name,
          tool_call_id: toolCallId,
          status: 'error',
          content: '調査用 tool 呼び出しは制限時間で中断されました。',
        }),
    ),
  ]
}
