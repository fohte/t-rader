import { ToolMessage } from '@langchain/core/messages'
import { createMiddleware } from 'langchain'

export const createToolSuccessMiddleware = (
  onToolSuccess: (toolName: string) => void,
) =>
  createMiddleware({
    name: 'toolSuccessMiddleware',
    wrapToolCall: async (request, handler) => {
      const result = await handler(request)
      if (ToolMessage.isInstance(result) && result.status !== 'error') {
        onToolSuccess(request.toolCall.name)
      }
      return result
    },
  })
