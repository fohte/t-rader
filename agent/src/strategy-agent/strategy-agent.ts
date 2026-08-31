import type { Message } from '@a2a-js/sdk'
import { createGenAiTracingMiddleware } from '@fohte/service-kit/langchain-genai'
import { captureWithFingerprint } from '@fohte/service-kit/observability'
import type { BaseChatModel } from '@langchain/core/language_models/chat_models'
import { HumanMessage, SystemMessage } from '@langchain/core/messages'
import type { DynamicStructuredTool } from '@langchain/core/tools'
import { MultiServerMCPClient } from '@langchain/mcp-adapters'
import { ChatOpenAI } from '@langchain/openai'
import {
  createAgent,
  toolErrorMiddleware,
  ToolInvocationError,
  toolStrategy,
} from 'langchain'
import { errAsync, ResultAsync } from 'neverthrow'
import { z } from 'zod'

import { extractMessageText } from '#a2a/message-text'
import type { FetchAgentConfig } from '#strategy-agent/agent-config-client'
import { createAgentConfigFetcher } from '#strategy-agent/agent-config-client'
import { parseAgentGraph } from '#strategy-agent/agent-graph/parse'
import type {
  BuildPhaseAgentOptions,
  CompiledPhaseAgent,
} from '#strategy-agent/agent-graph/run-agent-graph'
import { runAgentGraph } from '#strategy-agent/agent-graph/run-agent-graph'
import type { StrategyTaskStep } from '#strategy-agent/agent-graph/step'
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
  invoke(input: { messages: readonly HumanMessage[] }): Promise<{
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
  readonly buildPhaseAgent: (
    options: BuildPhaseAgentOptions,
  ) => CompiledPhaseAgent
}

export interface StrategyAgentConfig {
  readonly backendApiBaseUrl: string
  readonly strategyMcpUrl: string
  readonly llmApiKey: string
  readonly llmBaseUrl?: string | undefined
  readonly genAiProviderName: string
}

// createDefaultBuildAgent/createDefaultBuildPhaseAgent (後述) の共通処理。
// 両者は渡す response schema が異なるだけ。createAgent 自体の型推論は
// `responseFormat: ReturnType<typeof toolStrategy>` を呼び出し側のスキーマに
// 関わらず `Record<string, unknown>` に collapse するため、この関数は常に
// その erase された形を返す。createDefaultBuildAgent 側で自身の固定スキーマに
// narrowing し直す。
const buildCompiledAgent = (
  genAiProviderName: string,
  options: {
    model: BaseChatModel
    tools: readonly DynamicStructuredTool[]
    systemPrompt: string
    responseFormat: ReturnType<typeof toolStrategy>
  },
): CompiledPhaseAgent => {
  const agent = createAgent({
    model: options.model,
    tools: [...options.tools],
    // createAgent は string の systemPrompt を content parts の配列に変換して
    // SystemMessage を組み立てる (langchain 内部の normalizeSystemPrompt)。
    // chatgpt/* (LiteLLM 経由の Responses API bridge) は system/developer
    // ロールで content が配列だと 400 を返すため、SystemMessage インスタンスを
    // 渡して content を文字列のまま保つ。
    systemPrompt: new SystemMessage(options.systemPrompt),
    responseFormat: options.responseFormat,
    middleware: [
      createGenAiTracingMiddleware({ providerName: genAiProviderName }),
      // wrapToolCall middleware (added by the tracing middleware above for
      // its execute_tool span) makes LangChain's ToolNode stop
      // auto-recovering thrown tool errors into a ToolMessage, so this
      // restores that recovery explicitly, matching ToolNode's own default
      // handleToolErrors text (`${error}\n Please fix your mistakes.`).
      // ToolInvocationError (tool-input schema validation failures) is
      // passed through as-is since its message already ends with its own
      // "fix and retry" instruction.
      toolErrorMiddleware({
        onError: (error) =>
          ToolInvocationError.isInstance(error)
            ? String(error)
            : `${String(error)}\n Please fix your mistakes.`,
      }),
    ],
  })
  return {
    invoke: async (input) => {
      const result = await agent.invoke({ messages: [...input.messages] })
      // createAgent の推論型では structuredResponse は常に存在する扱いだが、
      // 実行時はモデルが structured-output tool を呼び出さないこともある。
      // このキャストでその可能性を型上に戻し、直後の undefined チェックが
      // unreachable と判定されないようにする。
      const structuredResponse = result.structuredResponse as
        Record<string, unknown> | undefined
      // exactOptionalPropertyTypes により optional property に対して
      // `{ structuredResponse: undefined }` を渡すことは許されないため、
      // エージェントが返さなかった場合はキー自体を省略する。
      if (structuredResponse === undefined) return {}
      return { structuredResponse }
    },
  }
}

const createDefaultBuildAgent =
  (genAiProviderName: string) =>
  (options: BuildStrategyAgentOptions): CompiledStrategyAgent => {
    const compiled = buildCompiledAgent(genAiProviderName, {
      ...options,
      responseFormat: toolStrategy(structuredResponseSchema),
    })
    return {
      invoke: async (input) => {
        const result = await compiled.invoke(input)
        if (result.structuredResponse === undefined) return {}
        return {
          // eslint-disable-next-line @typescript-eslint/no-unsafe-type-assertion -- buildCompiledAgent は erase された Record<string, unknown> 形しか知らない (doc comment 参照) ため、上で toolStrategy に渡したスキーマへここで narrowing し直す。
          structuredResponse: result.structuredResponse as z.infer<
            typeof structuredResponseSchema
          >,
        }
      },
    }
  }

// createDefaultBuildAgent と同じ形だが、response schema は固定の
// {status, message} zod スキーマではなく、agent_graph の `output` 設定から
// フェーズごとに組み立てた生の JSON Schema — toolStrategy はどちらも
// 受け付ける。CompiledPhaseAgent 自身の structuredResponse 型は既に erase
// された Record<string, unknown> 形のため、narrowing は不要。
const createDefaultBuildPhaseAgent =
  (genAiProviderName: string) =>
  (options: BuildPhaseAgentOptions): CompiledPhaseAgent =>
    buildCompiledAgent(genAiProviderName, {
      ...options,
      responseFormat: toolStrategy(options.responseSchema),
    })

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
      // model 引数によっては上流 backend が非ストリーミングの応答から
      // output を復元できず呼び出しが失敗するため、常に streaming で呼ぶ。
      streaming: true,
      configuration: {
        baseURL: config.llmBaseUrl ?? OPENCODE_GO_BASE_URL,
      },
    }),
  buildAgent: createDefaultBuildAgent(config.genAiProviderName),
  buildPhaseAgent: createDefaultBuildPhaseAgent(config.genAiProviderName),
})

export const runStrategyAgent = async (
  deps: StrategyAgentDeps,
  strategyId: string,
  userMessage: Message,
  onStepsChanged?: (steps: readonly StrategyTaskStep[]) => void,
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
            const parsedGraph = parseAgentGraph(agentConfig.agentGraph)
            if (parsedGraph.isErr()) {
              return errAsync(parsedGraph.error)
            }
            // agent_graph が設定されている場合は多段フェーズのオーケストレー
            // ターに委譲する。これは既に StrategyAgentResult に resolve
            // される (reject はしないが、fromPromise を通すことで想定外の
            // throw も下の toErrorResult と同じ経路に流す)。
            if (parsedGraph.value !== undefined) {
              return ResultAsync.fromPromise(
                runAgentGraph(deps, parsedGraph.value, {
                  agentsMd: agentConfig.agentsMd,
                  skills: agentConfig.skills,
                  tools,
                  originalPromptText: extractMessageText(userMessage),
                  ...(onStepsChanged !== undefined ? { onStepsChanged } : {}),
                }).then((result) => {
                  if (result.status === 'failed') {
                    console.error(
                      'strategy agent execution failed:',
                      result.message,
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
            }

            const agent = deps.buildAgent({
              model: deps.createChatModel(agentConfig.model),
              tools,
              systemPrompt: buildSystemPrompt(agentConfig),
            })
            return ResultAsync.fromPromise(
              agent.invoke({
                messages: [new HumanMessage(extractMessageText(userMessage))],
              }),
              (error) => error,
            ).map((invokeResult): StrategyAgentResult => {
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
          }),
        )
        .match((r) => r, toErrorResult)
    })
    .catch((error: unknown) => toErrorResult(error))
    .finally(() => closeMcpClient())
}
