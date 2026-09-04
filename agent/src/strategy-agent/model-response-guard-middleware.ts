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

// モデル応答が構造化出力の提出または宣言済み tool 呼び出しの契約を満たしているかを
// 検知し、ログと Sentry capture で可視化する。挙動は変えない (agent-graph 経由の
// 構造化出力再試行は run-agent-graph.ts、非 agent-graph の単一呼び出しでは
// 即失敗、宣言外 tool 呼び出しへの応答は langchain の ToolNode が担う)。
export const modelResponseGuardMiddleware = createMiddleware({
  name: 'modelResponseGuardMiddleware',
  wrapModelCall: async (request, handler) => {
    const response = await handler(request)
    // createAgent が構造化出力を直接解決した場合は AIMessage ではなく
    // { structuredResponse, messages } が返るため、検証をスキップする。
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
