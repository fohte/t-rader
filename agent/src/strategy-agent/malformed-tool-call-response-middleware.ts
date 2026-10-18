import { AIMessage, AIMessageChunk } from '@langchain/core/messages'
import { createMiddleware } from 'langchain'
import { Result } from 'neverthrow'

import { isRecord } from '#errors'

const MALFORMED_TOOL_CALL_MARKUP = /<\/?(?:tool_call|function|parameter)\b/i

const parseJson = Result.fromThrowable(
  (value: string): unknown => JSON.parse(value),
  (cause): unknown => cause,
)

const hasMalformedToolCallArguments = (response: AIMessage): boolean =>
  AIMessageChunk.isInstance(response) &&
  (response.tool_call_chunks ?? []).some((toolCallChunk) => {
    const args = toolCallChunk.args
    return args !== undefined && parseJson(args).isErr()
  })

const messageText = (content: unknown): string => {
  const parts = Array.isArray(content) ? content : [content]
  return parts
    .map((part) =>
      typeof part === 'string'
        ? part
        : isRecord(part) && typeof part['text'] === 'string'
          ? part['text']
          : '',
    )
    .join('')
}

const malformedReason = (response: unknown): string | undefined => {
  if (!AIMessage.isInstance(response)) return undefined
  if (hasMalformedToolCallArguments(response)) {
    return 'tool call arguments are not valid JSON'
  }
  if ((response.invalid_tool_calls ?? []).length > 0) {
    return 'invalid tool calls are present'
  }
  if (MALFORMED_TOOL_CALL_MARKUP.test(messageText(response.content))) {
    return 'tool call markup is present in message content'
  }
  return undefined
}

export class MalformedToolCallResponseError extends Error {
  constructor(reason: string) {
    super(`malformedToolCallResponseMiddleware: ${reason}`)
    this.name = 'MalformedToolCallResponseError'
  }
}

export const malformedToolCallResponseMiddleware = createMiddleware({
  name: 'malformedToolCallResponseMiddleware',
  wrapModelCall: async (request, handler) => {
    const response = await handler(request)
    const reason = malformedReason(response)
    if (reason !== undefined) {
      return Promise.reject(new MalformedToolCallResponseError(reason))
    }
    return response
  },
})
