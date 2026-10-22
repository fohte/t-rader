import { MultiServerMCPClient } from '@langchain/mcp-adapters'
import { ChatOpenAI } from '@langchain/openai'
import { errAsync, ResultAsync } from 'neverthrow'

import { isRecord } from '#errors'
import { createAgentConfigFetcher } from '#strategy-agent/agent-config-client'
import { createDefaultBuildPhaseAgent } from '#strategy-agent/compiled-phase-agent'
import type { ExecutionToolRequest } from '#strategy-agent/execution-tool-client'
import { parseExecutionToolResult } from '#strategy-agent/execution-tool-client'
import { createReasoningContentAwareFetch } from '#strategy-agent/reasoning-content-middleware'
import type {
  StrategyAgentConfig,
  StrategyAgentDeps,
} from '#strategy-agent/strategy-agent'
import { createSubagentDefinitionFetcher } from '#strategy-agent/subagent-definition-client'
import { createTaskExecutionSnapshotFetcher } from '#strategy-agent/task-execution-snapshot'

// OpenCode Go's OpenAI-compatible endpoint.
const OPENCODE_GO_BASE_URL = 'https://opencode.ai/zen/go/v1'

const STRATEGY_ID_HEADER = 'x-strategy-id'
const STRATEGY_TASK_ID_HEADER = 'x-strategy-task-id'
const EXECUTION_ID_HEADER = 'x-execution-id'
const SUBAGENT_EXECUTION_ID_HEADER = 'x-subagent-execution-id'
const SUBAGENT_DEFINITION_KEY_HEADER = 'x-subagent-definition-key'
const TOOL_MODELS_HEADER = 'x-tool-models'
const MCP_TOOL_TIMEOUT_MS = 150_000

type ReasoningEffort = NonNullable<
  NonNullable<
    NonNullable<ConstructorParameters<typeof ChatOpenAI>[0]>['reasoning']
  >['effort']
>

// Real wiring for production use; tests inject StrategyAgentDeps directly.
export const createStrategyAgentDeps = (
  config: StrategyAgentConfig,
): StrategyAgentDeps => {
  const createMcpClient: StrategyAgentDeps['createMcpClient'] = (
    strategyId,
    strategyTaskId,
    executionId,
    toolModels,
    subagentExecution,
  ) => {
    const client = new MultiServerMCPClient({
      defaultToolTimeout: MCP_TOOL_TIMEOUT_MS,
      mcpServers: {
        strategy: {
          url: `${config.backendApiBaseUrl.replace(/\/+$/, '')}/mcp/strategy`,
          headers: {
            [STRATEGY_ID_HEADER]: strategyId,
            [STRATEGY_TASK_ID_HEADER]: strategyTaskId,
            [EXECUTION_ID_HEADER]: executionId,
            [TOOL_MODELS_HEADER]: JSON.stringify(toolModels),
            ...(subagentExecution !== undefined
              ? {
                  [SUBAGENT_EXECUTION_ID_HEADER]: subagentExecution.executionId,
                  [SUBAGENT_DEFINITION_KEY_HEADER]:
                    subagentExecution.definitionKey,
                }
              : {}),
          },
        },
      },
    })
    const callBudgetOperation = async (
      argumentsValue: Record<string, unknown>,
    ): Promise<Record<string, unknown>> => {
      const strategyClient = await client.getClient('strategy')
      if (strategyClient === undefined) {
        return Promise.reject(new Error('strategy MCP client is unavailable'))
      }
      const result = await strategyClient.callTool({
        name: '__execution_budget',
        arguments: argumentsValue,
      })
      if (result.isError === true) {
        return Promise.reject(new Error('execution budget operation failed'))
      }
      if (!isRecord(result.structuredContent)) {
        return Promise.reject(new Error('execution budget response is invalid'))
      }
      return result.structuredContent
    }
    return {
      getTools: () => client.getTools(),
      callExecutionTool: async (
        request: ExecutionToolRequest,
        signal?: AbortSignal,
      ) =>
        await ResultAsync.fromPromise(
          client.getClient('strategy'),
          (error) =>
            new Error('failed to connect to strategy MCP server', {
              cause: error,
            }),
        ).andThen((mcpClient) =>
          mcpClient === undefined
            ? errAsync(new Error('failed to connect to strategy MCP server'))
            : ResultAsync.fromPromise(
                mcpClient.callTool(
                  request,
                  undefined,
                  signal === undefined ? undefined : { signal },
                ),
                (error) =>
                  new Error(`${request.name} MCP tool call failed`, {
                    cause: error,
                  }),
              ).andThen(parseExecutionToolResult),
        ),
      reserveModelCall: async (callId) => {
        const result = await callBudgetOperation({
          action: 'reserve_model_call',
          call_id: callId,
        })
        if (result['available'] !== true) {
          return Promise.reject(
            new Error('model call reservation is unavailable'),
          )
        }
      },
      reserveSubagentRun: async (input) => {
        const result = await callBudgetOperation({
          action: 'reserve_subagent_run',
          call_id: input.callId,
          phase_key: input.phaseKey,
          parent_execution_step_id: input.parentExecutionStepId,
          definition_key: input.definitionKey,
        })
        if (result['available'] !== true) {
          return Promise.reject(
            new Error('subagent run reservation is unavailable'),
          )
        }
      },
      transitionReservation: async (callId, state) => {
        const result = await callBudgetOperation({
          action: 'transition',
          call_id: callId,
          state,
        })
        if (state === 'consumed' && result['transitioned'] !== true) {
          return Promise.reject(
            new Error('execution reservation was already claimed'),
          )
        }
      },
      close: () => client.close(),
    }
  }

  return {
    fetchAgentConfig: createAgentConfigFetcher(config.backendApiBaseUrl),
    fetchTaskExecutionSnapshot:
      createTaskExecutionSnapshotFetcher(createMcpClient),
    fetchSubagentDefinition: createSubagentDefinitionFetcher(
      config.backendApiBaseUrl,
    ),
    createMcpClient,
    createChatModel: (model, options) =>
      new ChatOpenAI({
        apiKey: config.llmApiKey,
        model,
        // model 引数によっては上流 backend が非ストリーミングの応答から
        // output を復元できず呼び出しが失敗するため、常に streaming で呼ぶ。
        streaming: true,
        timeout: config.llmCallTimeoutMs,
        // SDK 内の再試行によるタイムアウトの累積を防ぎ、再試行を middleware に集約する。
        maxRetries: 0,
        configuration: {
          baseURL: config.llmBaseUrl ?? OPENCODE_GO_BASE_URL,
          fetch: createReasoningContentAwareFetch(),
        },
        ...(options?.reasoningEffort !== undefined
          ? {
              reasoning: {
                // eslint-disable-next-line @typescript-eslint/no-unsafe-type-assertion -- agent_graph 由来の reasoningEffort は model と同じ自由文字列として持ち回っており、ChatOpenAI 側のリテラル型に合わせ込むための意図的な narrowing。
                effort: options.reasoningEffort as ReasoningEffort,
              },
            }
          : {}),
      }),
    buildPhaseAgent: createDefaultBuildPhaseAgent(
      config.genAiProviderName,
      config.llmCallTimeoutMs,
    ),
  }
}
