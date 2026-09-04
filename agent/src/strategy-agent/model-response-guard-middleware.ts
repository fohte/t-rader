import { captureWithFingerprint } from '@fohte/service-kit/observability'
import { AIMessage } from '@langchain/core/messages'
import { createMiddleware, ToolStrategy } from 'langchain'

const MISSING_STRUCTURED_OUTPUT_FINGERPRINT =
  'model-response-guard-middleware.missing-structured-output'
const UNDECLARED_TOOL_CALL_FINGERPRINT =
  'model-response-guard-middleware.undeclared-tool-call'

// ServerTool は素の Record<string, unknown> で name を持つとは限らないため、
// 実行時にフィールドの有無を見て判定する (ClientTool は必ず name を持つ)。
const toolNameOf = (tool: unknown): string | undefined =>
  typeof tool === 'object' &&
  tool !== null &&
  'name' in tool &&
  typeof tool.name === 'string'
    ? tool.name
    : undefined

// モデルの応答が契約 (構造化出力を返す/宣言済み tool しか呼ばない) を満たして
// いるかを、モデル呼び出し直後にログと Sentry capture で可視化する。挙動は
// 変えず検知のみ行う: 構造化出力の再試行は run-agent-graph.ts、宣言外 tool 呼び出し
// への応答は langchain の ToolNode (エラーメッセージを返して再試行させる) が担う。
export const modelResponseGuardMiddleware = createMiddleware({
  name: 'modelResponseGuardMiddleware',
  wrapModelCall: async (request, handler) => {
    const response = await handler(request)
    // handler() の型は AIMessage を謳うが、モデルが構造化出力 tool を単独で
    // 呼んで解決した場合は createAgent 内部の { structuredResponse, messages }
    // 形状 (AIMessage ではない) がそのまま返ってくる。この形状は構造化出力が
    // 既に得られたことの証なので検証不要。
    if (!AIMessage.isInstance(response)) return response
    const toolCalls = response.tool_calls ?? []

    if (toolCalls.length === 0) {
      const error = new Error(
        'modelResponseGuardMiddleware: model call ended without a structured-output tool call',
      )
      console.warn(error.message)
      captureWithFingerprint(error, MISSING_STRUCTURED_OUTPUT_FINGERPRINT)
      return response
    }

    const declaredToolNames = new Set(
      request.tools
        .map(toolNameOf)
        .filter((name): name is string => name !== undefined),
    )
    const structuredOutputToolNames = new Set(
      (Array.isArray(request.responseFormat) ? request.responseFormat : [])
        .filter(
          (format): format is ToolStrategy => format instanceof ToolStrategy,
        )
        .map((format) => format.name),
    )
    const undeclaredNames = [
      ...new Set(
        toolCalls
          .map((call) => call.name)
          .filter(
            (name) =>
              !declaredToolNames.has(name) &&
              !structuredOutputToolNames.has(name),
          ),
      ),
    ]
    if (undeclaredNames.length > 0) {
      const error = new Error(
        `modelResponseGuardMiddleware: model called undeclared tool(s): ${undeclaredNames.join(', ')}`,
      )
      console.warn(error.message)
      captureWithFingerprint(error, UNDECLARED_TOOL_CALL_FINGERPRINT, {
        extras: { undeclaredNames },
      })
    }

    return response
  },
})
