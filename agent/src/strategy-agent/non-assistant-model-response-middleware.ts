import { AIMessage, BaseMessage } from '@langchain/core/messages'
import { createMiddleware } from 'langchain'

export class NonAssistantModelResponseError extends Error {
  constructor(messageType: string) {
    super(
      `nonAssistantModelResponseMiddleware: model returned a non-assistant message (type: ${messageType})`,
    )
    this.name = 'NonAssistantModelResponseError'
  }
}

const nonAssistantMessageType = (response: unknown): string | undefined => {
  if (!BaseMessage.isInstance(response) || AIMessage.isInstance(response)) {
    return undefined
  }
  return response.type
}

export const nonAssistantModelResponseMiddleware = createMiddleware({
  name: 'nonAssistantModelResponseMiddleware',
  wrapModelCall: async (request, handler) => {
    const response = await handler(request)
    const messageType = nonAssistantMessageType(response)
    if (messageType !== undefined) {
      return Promise.reject(new NonAssistantModelResponseError(messageType))
    }
    return response
  },
})
