import type { Message } from '@a2a-js/sdk'
import { captureWithFingerprint } from '@fohte/service-kit/observability'
import type { BaseChatModel } from '@langchain/core/language_models/chat_models'
import type { DynamicStructuredTool } from '@langchain/core/tools'
import { errAsync, ResultAsync } from 'neverthrow'

import { extractMessageText } from '#a2a/message-text'
import { logger } from '#logger'
import type { FetchAgentConfig } from '#strategy-agent/agent-config-client'
import { parseAgentGraph } from '#strategy-agent/agent-graph/parse'
import type {
  BuildPhaseAgentOptions,
  CompiledPhaseAgent,
} from '#strategy-agent/agent-graph/run-agent-graph'
import { runAgentGraph } from '#strategy-agent/agent-graph/run-agent-graph'
import type { StrategyTaskStep } from '#strategy-agent/agent-graph/step'
import {
  fromStepJson,
  strategyTaskStepJsonSchema,
} from '#strategy-agent/agent-graph/step'
import { withAsOf } from '#strategy-agent/as-of'
import type { FetchSubagentDefinition } from '#strategy-agent/subagent-definition-client'
import { createRunSubagentTool } from '#strategy-agent/subagent-tool'
import { isUsageLimitError } from '#strategy-agent/usage-limit'

const DEFAULT_PURPOSE = 'default'

const EXECUTION_FAILED_FINGERPRINT = 'strategy-agent.execution-failed'
const RESUME_STEPS_PARSE_FAILED_FINGERPRINT =
  'strategy-agent.resume-steps-parse-failed'

export interface StrategyAgentResult {
  readonly status: 'completed' | 'failed'
  readonly message: string
  readonly errorKind?: 'usage_limit' | 'agent_error'
}

export interface McpToolsClient {
  getTools(): Promise<DynamicStructuredTool[]>
  reserveModelCall(callId: string): Promise<void>
  reserveSubagentRun(input: {
    readonly callId: string
    readonly phaseKey: string
    readonly parentExecutionStepId: string
    readonly definitionKey: string
  }): Promise<void>
  transitionReservation(
    callId: string,
    state: 'consumed' | 'released',
  ): Promise<void>
  close(): Promise<void>
}

interface SubagentExecutionIdentity {
  readonly executionId: string
  readonly definitionKey: string
}

export interface StrategyAgentDeps {
  readonly fetchAgentConfig: FetchAgentConfig
  readonly fetchSubagentDefinition: FetchSubagentDefinition
  readonly createMcpClient: (
    strategyId: string,
    strategyTaskId: string,
    executionId: string,
    toolModels: Readonly<Record<string, string>>,
    subagentExecution?: SubagentExecutionIdentity,
  ) => McpToolsClient
  readonly createChatModel: (
    model: string,
    options?: { reasoningEffort?: string },
  ) => BaseChatModel
  readonly buildPhaseAgent: (
    options: BuildPhaseAgentOptions,
  ) => CompiledPhaseAgent
}

export interface StrategyAgentConfig {
  readonly backendApiBaseUrl: string
  readonly llmApiKey: string
  readonly llmBaseUrl?: string | undefined
  readonly genAiProviderName: string
  readonly llmCallTimeoutMs: number
}

export { createStrategyAgentDeps } from '#strategy-agent/strategy-agent-deps'

// パースに失敗した要素は無視し、正常にパースできたステップのみ再開情報として扱う。
const parseResumeSteps = (
  resumeSteps: unknown[] | undefined,
  strategyId: string,
): StrategyTaskStep[] | undefined => {
  if (resumeSteps === undefined) return undefined
  const results = resumeSteps.map((raw) =>
    strategyTaskStepJsonSchema.safeParse(raw),
  )
  const failures = results.filter((result) => !result.success)
  if (failures.length > 0) {
    const error = new Error(
      `${String(failures.length)} of ${String(results.length)} resume step(s) failed schema validation`,
    )
    logger.error(
      { issues: failures.flatMap((f) => f.error.issues) },
      'failed to parse resume steps',
    )
    captureWithFingerprint(error, RESUME_STEPS_PARSE_FAILED_FINGERPRINT, {
      extras: { strategyId, failedCount: failures.length },
    })
  }
  return results
    .filter((result) => result.success)
    .map((result) => fromStepJson(result.data))
}

export interface RunStrategyAgentInput {
  readonly strategyId: string
  readonly purpose: string | undefined
  readonly taskId: string
  readonly strategyTaskId: string
  readonly userMessage: Message
  readonly resumeSteps: unknown[] | undefined
  readonly asOf: Date | undefined
  readonly deadlineSignal: AbortSignal | undefined
  readonly onStepsChanged?: (steps: readonly StrategyTaskStep[]) => void
}

export const runStrategyAgent = async (
  deps: StrategyAgentDeps,
  input: RunStrategyAgentInput,
): Promise<StrategyAgentResult> => {
  const {
    strategyId,
    purpose,
    taskId,
    strategyTaskId,
    userMessage,
    resumeSteps,
    asOf,
    deadlineSignal,
    onStepsChanged,
  } = input
  const promptText = withAsOf(extractMessageText(userMessage), asOf)
  const previousSteps = parseResumeSteps(resumeSteps, strategyId)

  const toErrorResult = (error: unknown): StrategyAgentResult => {
    logger.error({ err: error }, 'strategy agent execution failed')
    captureWithFingerprint(error, EXECUTION_FAILED_FINGERPRINT, {
      extras: { strategyId },
    })
    if (isUsageLimitError(error)) {
      return {
        status: 'failed',
        message: 'usage limit reached',
        errorKind: 'usage_limit',
      }
    }
    return {
      status: 'failed',
      message: error instanceof Error ? error.message : String(error),
      errorKind: 'agent_error',
    }
  }

  return Promise.resolve()
    .then(() =>
      deps
        .fetchAgentConfig({ purpose: purpose ?? DEFAULT_PURPOSE })
        .andThen((agentConfig) => {
          const parsedGraph = parseAgentGraph(agentConfig.agentGraph)
          if (parsedGraph.isErr()) {
            return errAsync(parsedGraph.error)
          }
          return ResultAsync.fromPromise(
            runAgentGraph(deps, parsedGraph.value, {
              agentsMd: agentConfig.agentsMd,
              skills: agentConfig.skills,
              createStepMcpClient: (executionStepId) =>
                deps.createMcpClient(
                  strategyId,
                  strategyTaskId,
                  `${taskId}:${executionStepId}`,
                  parsedGraph.value.toolModels ?? {},
                ),
              createSubagentTool: (
                phase,
                parentExecutionStepId,
                recorder,
                phaseContext,
                parentMcpClient,
              ) => {
                return createRunSubagentTool({
                  phase,
                  parentExecutionStepId,
                  recorder,
                  parentMcpClient,
                  fetchDefinition: deps.fetchSubagentDefinition,
                  createMcpClient: (executionStepId, definitionKey) =>
                    deps.createMcpClient(
                      strategyId,
                      strategyTaskId,
                      `${taskId}:${executionStepId}`,
                      parsedGraph.value.toolModels ?? {},
                      { executionId: executionStepId, definitionKey },
                    ),
                  deps,
                  ...(phaseContext.deadlineSignal !== undefined
                    ? { deadlineSignal: phaseContext.deadlineSignal }
                    : {}),
                  ...(phaseContext.phaseTimeout !== undefined
                    ? { phaseTimeout: phaseContext.phaseTimeout }
                    : {}),
                })
              },
              originalPromptText: promptText,
              ...(onStepsChanged !== undefined ? { onStepsChanged } : {}),
              ...(previousSteps !== undefined ? { previousSteps } : {}),
              ...(deadlineSignal !== undefined ? { deadlineSignal } : {}),
            }).then((result) => {
              if (result.status === 'failed') {
                logger.error(
                  { error: result.message },
                  'strategy agent execution failed',
                )
                captureWithFingerprint(
                  new Error(result.message),
                  EXECUTION_FAILED_FINGERPRINT,
                  { extras: { strategyId } },
                )
              }
              return result
            }),
            (error) => error,
          )
        })
        .match((r) => r, toErrorResult),
    )
    .catch((error: unknown) => toErrorResult(error))
}
