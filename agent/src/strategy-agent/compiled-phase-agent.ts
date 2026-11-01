import { createGenAiTracingMiddleware } from '@fohte/service-kit/langchain-genai'
import type { BaseChatModel } from '@langchain/core/language_models/chat_models'
import { BaseMessage, SystemMessage } from '@langchain/core/messages'
import type { DynamicStructuredTool } from '@langchain/core/tools'
import { createAgent, toolStrategy } from 'langchain'

import { isPlainObject } from '#strategy-agent/agent-graph/json'
import type {
  BuildPhaseAgentOptions,
  CompiledPhaseAgent,
} from '#strategy-agent/agent-graph/run-agent-graph'
import { createCallDurationMiddleware } from '#strategy-agent/call-duration-middleware'
import { createDeadlineMiddleware } from '#strategy-agent/deadline-middleware'
import {
  createFinalTurnMiddleware,
  finalTurnMiddleware,
  MAX_MODEL_CALLS_PER_INVOKE,
  type ModelCallBudget,
} from '#strategy-agent/final-turn-middleware'
import { malformedToolCallResponseMiddleware } from '#strategy-agent/malformed-tool-call-response-middleware'
import { createMcpToolJsonEnvelopeMiddleware } from '#strategy-agent/mcp-tool-json-envelope-middleware'
import { createModelCallRetryMiddleware } from '#strategy-agent/model-call-retry-middleware'
import { modelResponseGuardMiddleware } from '#strategy-agent/model-response-guard-middleware'
import { nonAssistantModelResponseMiddleware } from '#strategy-agent/non-assistant-model-response-middleware'
import { reasoningContentMiddleware } from '#strategy-agent/reasoning-content-middleware'
import { stripMessageNameMiddleware } from '#strategy-agent/strip-message-name-middleware'
import {
  createToolCallCapMiddleware,
  MAX_TOOL_CALLS_PER_MODEL_CALL,
} from '#strategy-agent/tool-call-cap-middleware'
import { createStrategyToolErrorMiddleware } from '#strategy-agent/tool-error-middleware'
import { createToolSuccessMiddleware } from '#strategy-agent/tool-success-middleware'

// createAgent の型推論は response schema に関わらず
// `Record<string, unknown>` に collapse するため、その形で返す。
const buildCompiledAgent = (
  genAiProviderName: string,
  llmCallTimeoutMs: number,
  options: {
    model: BaseChatModel
    tools: readonly DynamicStructuredTool[]
    mcpTools: readonly DynamicStructuredTool[]
    systemPrompt: string
    responseFormat: ReturnType<typeof toolStrategy>
    deadlineSignal?: AbortSignal
    agentSignal?: AbortSignal
    phaseTimeout?: BuildPhaseAgentOptions['phaseTimeout']
    modelCallBudget?: ModelCallBudget
    reserveModelCall?: (callId: string) => Promise<void>
    transitionReservation?: (
      callId: string,
      state: 'consumed' | 'released',
    ) => Promise<void>
    captureMessagesOnAbort?: boolean
    onModelCallLimitReached?: () => void
    onToolSuccess?: BuildPhaseAgentOptions['onToolSuccess']
  },
): CompiledPhaseAgent => {
  const toolNames = new Set(options.tools.map((tool) => tool.name))
  const maxModelCalls =
    options.modelCallBudget?.maxModelCalls ?? MAX_MODEL_CALLS_PER_INVOKE
  const modelCallMiddleware =
    options.modelCallBudget !== undefined ||
    options.onModelCallLimitReached !== undefined
      ? createFinalTurnMiddleware(
          maxModelCalls,
          options.modelCallBudget,
          options.onModelCallLimitReached,
        )
      : finalTurnMiddleware
  const agent = createAgent({
    model: options.model,
    tools: [...options.tools],
    // createAgent は string の systemPrompt を content parts 配列に変換するが、
    // 上流 API の一部は system ロールの配列 content を拒否するため文字列のまま渡す。
    systemPrompt: new SystemMessage(options.systemPrompt),
    responseFormat: options.responseFormat,
    middleware: [
      createGenAiTracingMiddleware({ providerName: genAiProviderName }),
      ...(options.onToolSuccess !== undefined
        ? [
            createToolSuccessMiddleware((toolName) => {
              if (toolNames.has(toolName)) options.onToolSuccess?.(toolName)
            }),
          ]
        : []),
      // tracing middleware の wrapToolCall は tool error を自動復旧しないため、
      // 通常の ToolMessage に戻しつつ MCP の通信障害を記録する。
      createStrategyToolErrorMiddleware(),
      modelCallMiddleware,
      // finalTurnMiddleware より内側に置き、
      // 実際にモデルへ渡った tools と生の応答を見て契約違反を検知する。
      modelResponseGuardMiddleware,
      ...(options.mcpTools.length > 0
        ? [createMcpToolJsonEnvelopeMiddleware(options.mcpTools)]
        : []),
      stripMessageNameMiddleware,
      // 再試行ごとに呼び出し時間上限と tool 呼び出し上限の状態を作り直す。
      createModelCallRetryMiddleware(
        options.reserveModelCall,
        options.transitionReservation,
      ),
      malformedToolCallResponseMiddleware,
      // 実際の HTTP リクエストに一番近い位置で signal / callback を差し込む。
      createToolCallCapMiddleware(MAX_TOOL_CALLS_PER_MODEL_CALL),
      createCallDurationMiddleware(llmCallTimeoutMs),
      ...(options.deadlineSignal !== undefined
        ? [
            createDeadlineMiddleware(
              options.deadlineSignal,
              options.phaseTimeout,
            ),
          ]
        : []),
      reasoningContentMiddleware,
      // LangChain の戻り値検証前に assistant 以外の message を再試行可能なエラーへ変える。
      nonAssistantModelResponseMiddleware,
    ],
    ...(options.agentSignal !== undefined
      ? { signal: options.agentSignal }
      : {}),
  })
  return {
    invoke: async (input) => {
      const recursionLimit = maxModelCalls * 3
      if (options.captureMessagesOnAbort === true) {
        let latestState: {
          readonly messages: readonly BaseMessage[]
          readonly structuredResponse?: Record<string, unknown>
        } = { messages: [...input.messages] }
        const stream = agent
          .stream(
            { messages: [...input.messages] },
            { streamMode: 'values', recursionLimit },
          )
          .then(async (values) => {
            for await (const value of values) {
              latestState = parseAgentState(value) ?? latestState
            }
            return latestState
          })
        return stream.then(
          (state) => state,
          (error: unknown) => {
            if (options.deadlineSignal?.aborted === true) return latestState
            return Promise.reject(
              error instanceof Error ? error : new Error(String(error)),
            )
          },
        )
      }
      const result = await agent.invoke(
        { messages: [...input.messages] },
        {
          // finalTurnMiddleware が MAX_MODEL_CALLS_PER_INVOKE 到達ターンで
          // 提出を強制するため、graph 自体の recursionLimit (デフォルト 25) は
          // それより十分先に置き、実際に効くのは finalTurnMiddleware 側にする。
          recursionLimit,
        },
      )
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

const parseAgentState = (
  value: unknown,
):
  | {
      readonly messages: readonly BaseMessage[]
      readonly structuredResponse?: Record<string, unknown>
    }
  | undefined => {
  if (!isPlainObject(value)) return undefined
  const record = value
  const messages = Array.isArray(record['messages'])
    ? record['messages'].filter(
        (message): message is BaseMessage => message instanceof BaseMessage,
      )
    : []
  const structuredResponse = record['structuredResponse']
  return {
    messages,
    ...(isPlainObject(structuredResponse) ? { structuredResponse } : {}),
  }
}

// response schema は agent_graph の `output` 設定からフェーズごとに組み立てる。
export const createDefaultBuildPhaseAgent =
  (genAiProviderName: string, llmCallTimeoutMs: number) =>
  (options: BuildPhaseAgentOptions): CompiledPhaseAgent =>
    buildCompiledAgent(genAiProviderName, llmCallTimeoutMs, {
      ...options,
      responseFormat: toolStrategy(options.responseSchema),
    })
