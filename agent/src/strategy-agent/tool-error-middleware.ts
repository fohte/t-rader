import { captureWithFingerprint } from '@fohte/service-kit/observability'
import { toolErrorMiddleware, ToolInvocationError } from 'langchain'

const MCP_TOOL_ERROR_FINGERPRINT = 'strategy-agent.mcp-tool-error'

const isMcpTransportOrInternalError = (error: unknown): error is Error => {
  if (ToolInvocationError.isInstance(error) || !(error instanceof Error)) {
    return false
  }

  const { message } = error
  if (
    message.startsWith('MCP tool ') &&
    message.includes(' returned an error:')
  ) {
    return false
  }

  return (
    message.includes('Streamable HTTP error:') ||
    /\bHTTP\s+[45]\d{2}\b/i.test(message) ||
    /MCP error -32603:/i.test(message) ||
    /\bSession not found\b/i.test(message)
  )
}

export const createStrategyToolErrorMiddleware = () =>
  toolErrorMiddleware({
    onError: (error, request) => {
      if (isMcpTransportOrInternalError(error)) {
        const toolName = request.toolCall.name
        captureWithFingerprint(
          error,
          [MCP_TOOL_ERROR_FINGERPRINT, toolName, '{{ default }}'],
          { level: 'error', extras: { toolName } },
        )
      }

      return ToolInvocationError.isInstance(error)
        ? String(error)
        : `${String(error)}\n Please fix your mistakes.`
    },
  })
