import type { Message } from '@a2a-js/sdk'
import { captureWithFingerprint } from '@fohte/service-kit/observability'
import type { BaseCallbackHandler } from '@langchain/core/callbacks/base'
import type { BaseChatModel } from '@langchain/core/language_models/chat_models'
import { HumanMessage } from '@langchain/core/messages'
import type { DynamicStructuredTool } from '@langchain/core/tools'
import { MultiServerMCPClient } from '@langchain/mcp-adapters'
import { ChatOpenAI } from '@langchain/openai'
import { createAgent, toolStrategy } from 'langchain'
import { ResultAsync } from 'neverthrow'
import { z } from 'zod'

import { extractMessageText } from '#a2a/message-text'
import type { FetchAgentConfig } from '#strategy-agent/agent-config-client'
import { createAgentConfigFetcher } from '#strategy-agent/agent-config-client'
import { buildSystemPrompt } from '#strategy-agent/system-prompt'
import { isUsageLimitError } from '#strategy-agent/usage-limit'

// OpenCode Go's OpenAI-compatible endpoint.
const OPENCODE_GO_BASE_URL = 'https://opencode.ai/zen/go/v1'

const STRATEGY_ID_HEADER = 'x-strategy-id'

const EXECUTION_FAILED_FINGERPRINT = 'strategy-agent.execution-failed'
const MCP_CLIENT_CLOSE_FAILED_FINGERPRINT =
  'strategy-agent.mcp-client-close-failed'

const structuredResponseSchema = z.object({
  status: z.enum(['completed', 'error']),
  message: z.string(),
})

export interface StrategyAgentResult {
  readonly status: 'completed' | 'failed'
  readonly message: string
  readonly errorKind?: 'usage_limit' | 'agent_error'
}

export interface McpToolsClient {
  getTools(): Promise<DynamicStructuredTool[]>
  close(): Promise<void>
}

export interface CompiledStrategyAgent {
  invoke(
    input: { messages: readonly HumanMessage[] },
    options: { callbacks: readonly BaseCallbackHandler[] },
  ): Promise<{
    structuredResponse?: z.infer<typeof structuredResponseSchema>
  }>
}

export interface BuildStrategyAgentOptions {
  readonly model: BaseChatModel
  readonly tools: readonly DynamicStructuredTool[]
  readonly systemPrompt: string
}

export interface StrategyAgentDeps {
  readonly fetchAgentConfig: FetchAgentConfig
  readonly createMcpClient: (strategyId: string) => McpToolsClient
  readonly createChatModel: (model: string) => BaseChatModel
  readonly buildAgent: (
    options: BuildStrategyAgentOptions,
  ) => CompiledStrategyAgent
  readonly genAiCallbackHandler: BaseCallbackHandler
}

export interface StrategyAgentConfig {
  readonly backendApiBaseUrl: string
  readonly strategyMcpUrl: string
  readonly llmApiKey: string
  readonly llmBaseUrl?: string | undefined
  readonly genAiCallbackHandler: BaseCallbackHandler
}

const defaultBuildAgent = (
  options: BuildStrategyAgentOptions,
): CompiledStrategyAgent => {
  const agent = createAgent({
    model: options.model,
    tools: [...options.tools],
    systemPrompt: options.systemPrompt,
    responseFormat: toolStrategy(structuredResponseSchema),
  })
  return {
    invoke: async (input, callOptions) => {
      const result = await agent.invoke(
        { messages: [...input.messages] },
        { callbacks: [...callOptions.callbacks] },
      )
      return { structuredResponse: result.structuredResponse }
    },
  }
}

// Real wiring for production use; tests inject StrategyAgentDeps directly.
export const createStrategyAgentDeps = (
  config: StrategyAgentConfig,
): StrategyAgentDeps => ({
  fetchAgentConfig: createAgentConfigFetcher(config.backendApiBaseUrl),
  createMcpClient: (strategyId) =>
    new MultiServerMCPClient({
      mcpServers: {
        strategy: {
          url: config.strategyMcpUrl,
          headers: { [STRATEGY_ID_HEADER]: strategyId },
        },
      },
    }),
  createChatModel: (model) =>
    new ChatOpenAI({
      apiKey: config.llmApiKey,
      model,
      configuration: {
        baseURL: config.llmBaseUrl ?? OPENCODE_GO_BASE_URL,
      },
    }),
  buildAgent: defaultBuildAgent,
  genAiCallbackHandler: config.genAiCallbackHandler,
})

export const runStrategyAgent = async (
  deps: StrategyAgentDeps,
  strategyId: string,
  userMessage: Message,
): Promise<StrategyAgentResult> => {
  const mcpClient = deps.createMcpClient(strategyId)

  const closeMcpClient = (): Promise<void> =>
    mcpClient.close().catch((closeError: unknown) => {
      console.error('failed to close MCP client:', closeError)
      captureWithFingerprint(closeError, MCP_CLIENT_CLOSE_FAILED_FINGERPRINT, {
        extras: { strategyId },
      })
    })

  const toErrorResult = (error: unknown): StrategyAgentResult => {
    console.error('strategy agent execution failed:', error)
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

  // mcpClient is already constructed at this point, so chain construction
  // itself throwing synchronously (e.g. getTools() or fetchAgentConfig)
  // must still reach the .finally() below and close it. Wrapped in .then()
  // (rather than relying on this function's own `async` to convert a
  // synchronous throw to a rejection) makes that explicit.
  return Promise.resolve()
    .then(() => {
      // Started before fetchAgentConfig is awaited below so it's already
      // in flight rather than sequenced after it.
      const toolsResult = ResultAsync.fromPromise(
        mcpClient.getTools(),
        (error) => error,
      )

      // Chained via andThen (rather than Promise.all) so a fetchAgentConfig
      // failure fails fast instead of waiting out toolsResult first —
      // ResultAsync.fromPromise never rejects, so Promise.all would
      // otherwise wait for both to settle regardless of which one failed.
      return deps
        .fetchAgentConfig(strategyId)
        .andThen((agentConfig) =>
          toolsResult.andThen((tools) => {
            const agent = deps.buildAgent({
              model: deps.createChatModel(agentConfig.model),
              tools,
              systemPrompt: buildSystemPrompt(agentConfig),
            })
            return ResultAsync.fromPromise(
              agent.invoke(
                {
                  messages: [new HumanMessage(extractMessageText(userMessage))],
                },
                { callbacks: [deps.genAiCallbackHandler] },
              ),
              (error) => error,
            )
          }),
        )
        .map((invokeResult): StrategyAgentResult => {
          if (invokeResult.structuredResponse === undefined) {
            return {
              status: 'failed',
              message: 'agent did not return a structured response',
              errorKind: 'agent_error',
            }
          }
          if (invokeResult.structuredResponse.status === 'completed') {
            return {
              status: 'completed',
              message: invokeResult.structuredResponse.message,
            }
          }
          return {
            status: 'failed',
            message: invokeResult.structuredResponse.message,
            errorKind: 'agent_error',
          }
        })
        .match((r) => r, toErrorResult)
    })
    .catch((error: unknown) => toErrorResult(error))
    .finally(() => closeMcpClient())
}
