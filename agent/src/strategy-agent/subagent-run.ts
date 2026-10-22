import { DynamicStructuredTool } from '@langchain/core/tools'

import { buildOutputJsonSchema } from '#strategy-agent/agent-graph/output-schema'
import type {
  CompiledPhaseAgent,
  RunAgentGraphDeps,
  StepRecorder,
} from '#strategy-agent/agent-graph/run-agent-graph/types'
import type { AgentGraphPhase } from '#strategy-agent/agent-graph/types'
import type { ModelCallBudget } from '#strategy-agent/final-turn-middleware'
import type { PhaseTimeout } from '#strategy-agent/phase-timeout'
import type { McpToolsClient } from '#strategy-agent/strategy-agent'
import type {
  FetchSubagentDefinition,
  SubagentDefinition,
} from '#strategy-agent/subagent-definition-client'

export interface CreateRunSubagentToolOptions {
  readonly phase: AgentGraphPhase
  readonly parentExecutionStepId: string
  readonly recorder: StepRecorder
  readonly parentMcpClient: McpToolsClient
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

export const buildSubagentAgent = (
  definition: SubagentDefinition,
  deps: RunAgentGraphDeps,
  tools: readonly DynamicStructuredTool[],
  options: {
    readonly deadlineSignal?: AbortSignal
    readonly phaseTimeout?: PhaseTimeout
    readonly modelCallBudget: ModelCallBudget
    readonly reserveModelCall?: (callId: string) => Promise<void>
    readonly transitionReservation?: (
      callId: string,
      state: 'consumed' | 'released',
    ) => Promise<void>
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
    mcpTools: tools,
    systemPrompt: options.systemPrompt ?? definition.prompt,
    responseSchema: buildOutputJsonSchema(definition.output),
    modelCallBudget: options.modelCallBudget,
    ...(options.reserveModelCall !== undefined
      ? { reserveModelCall: options.reserveModelCall }
      : {}),
    ...(options.transitionReservation !== undefined
      ? { transitionReservation: options.transitionReservation }
      : {}),
    captureMessagesOnAbort: true,
    ...(options.deadlineSignal !== undefined
      ? {
          agentSignal: options.deadlineSignal,
          deadlineSignal: options.deadlineSignal,
        }
      : {}),
    ...(options.phaseTimeout !== undefined
      ? { phaseTimeout: options.phaseTimeout }
      : {}),
  })
