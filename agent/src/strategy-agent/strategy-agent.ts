import type { Message } from '@a2a-js/sdk'
import type { BaseCallbackHandler } from '@langchain/core/callbacks/base'
import type { BaseChatModel } from '@langchain/core/language_models/chat_models'
import { HumanMessage } from '@langchain/core/messages'
import type { DynamicStructuredTool } from '@langchain/core/tools'
import { MultiServerMCPClient } from '@langchain/mcp-adapters'
import { ChatOpenAI } from '@langchain/openai'
import { createAgent, toolStrategy } from 'langchain'
import { z } from 'zod'

import type { FetchAgentConfig } from '@/strategy-agent/agent-config-client'
import { createAgentConfigFetcher } from '@/strategy-agent/agent-config-client'
import { buildSystemPrompt } from '@/strategy-agent/system-prompt'
import { isUsageLimitError } from '@/strategy-agent/usage-limit'

// OpenCode Go's OpenAI-compatible endpoint.
const OPENCODE_GO_BASE_URL = 'https://opencode.ai/zen/go/v1'

const STRATEGY_ID_HEADER = 'x-strategy-id'

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
  readonly openCodeApiKey: string
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
      apiKey: config.openCodeApiKey,
      model,
      configuration: { baseURL: OPENCODE_GO_BASE_URL },
    }),
  buildAgent: defaultBuildAgent,
  genAiCallbackHandler: config.genAiCallbackHandler,
})

const extractMessageText = (message: Message): string =>
  message.parts
    .filter(
      (part): part is { kind: 'text'; text: string } => part.kind === 'text',
    )
    .map((part) => part.text)
    .join('\n')

export const runStrategyAgent = async (
  deps: StrategyAgentDeps,
  strategyId: string,
  userMessage: Message,
): Promise<StrategyAgentResult> => {
  const mcpClient = deps.createMcpClient(strategyId)

  try {
    const [agentConfig, tools] = await Promise.all([
      deps.fetchAgentConfig(strategyId),
      mcpClient.getTools(),
    ])

    const agent = deps.buildAgent({
      model: deps.createChatModel(agentConfig.model),
      tools,
      systemPrompt: buildSystemPrompt(agentConfig),
    })

    const result = await agent.invoke(
      { messages: [new HumanMessage(extractMessageText(userMessage))] },
      { callbacks: [deps.genAiCallbackHandler] },
    )

    if (result.structuredResponse === undefined) {
      return {
        status: 'failed',
        message: 'agent did not return a structured response',
        errorKind: 'agent_error',
      }
    }
    if (result.structuredResponse.status === 'completed') {
      return { status: 'completed', message: result.structuredResponse.message }
    }
    return {
      status: 'failed',
      message: result.structuredResponse.message,
      errorKind: 'agent_error',
    }
  } catch (error) {
    console.error('strategy agent execution failed:', error)
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
  } finally {
    // A close failure must not override the result/error already
    // determined above by discarding it in favor of this finally block's
    // own rejection.
    await mcpClient.close().catch((closeError: unknown) => {
      console.error('failed to close MCP client:', closeError)
    })
  }
}
