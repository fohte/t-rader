import { AsyncLocalStorage } from 'node:async_hooks'

import {
  AIMessage,
  type BaseMessage,
  ChatMessage,
} from '@langchain/core/messages'
import { ChatOpenAI } from '@langchain/openai'
import { createMiddleware } from 'langchain'
import { Result } from 'neverthrow'

import { isPlainObject } from '#strategy-agent/agent-graph/json'

export type ChatOpenAIFetch = NonNullable<
  NonNullable<ConstructorParameters<typeof ChatOpenAI>[0]>['configuration']
>['fetch']

const reasoningContentContext = new AsyncLocalStorage<readonly unknown[]>()

const parseJson = Result.fromThrowable(
  (body: string): unknown => JSON.parse(body),
  (cause): unknown => cause,
)

const stringifyJson = Result.fromThrowable(
  (value: Record<string, unknown>): string => JSON.stringify(value),
  (cause): unknown => cause,
)

const hasAudioId = (message: BaseMessage): boolean => {
  const audio = message.additional_kwargs['audio']
  return typeof audio === 'object' && audio !== null && 'id' in audio
}

const getAssistantReasoningContents = (
  messages: readonly BaseMessage[],
): readonly unknown[] => {
  const contents: unknown[] = []
  for (const message of messages) {
    if (
      AIMessage.isInstance(message) ||
      (ChatMessage.isInstance(message) && message.role === 'assistant')
    ) {
      contents.push(message.additional_kwargs['reasoning_content'])
    }
    if (hasAudioId(message)) contents.push(undefined)
  }
  return contents
}

export const reasoningContentMiddleware = createMiddleware({
  name: 'reasoningContentMiddleware',
  wrapModelCall: (request, handler) =>
    reasoningContentContext.run(
      getAssistantReasoningContents(request.messages),
      () => handler(request),
    ),
})

export const createReasoningContentAwareFetch =
  (fetchImpl: ChatOpenAIFetch = globalThis.fetch): ChatOpenAIFetch =>
  (input, init) => {
    const reasoningContents = reasoningContentContext.getStore()
    if (reasoningContents === undefined) return fetchImpl(input, init)

    const body = init?.body
    if (typeof body !== 'string') return fetchImpl(input, init)

    const parsed = parseJson(body)
    if (parsed.isErr() || !isPlainObject(parsed.value)) {
      return fetchImpl(input, init)
    }
    const requestMessages = parsed.value['messages']
    if (!Array.isArray(requestMessages)) return fetchImpl(input, init)

    let assistantMessageIndex = 0
    const messages = requestMessages.map((message: unknown) => {
      if (!isPlainObject(message) || message['role'] !== 'assistant') {
        return message
      }
      const reasoningContent = reasoningContents[assistantMessageIndex]
      assistantMessageIndex += 1
      if (reasoningContent === undefined) return message
      return { ...message, reasoning_content: reasoningContent }
    })
    if (
      messages.every((message, index) => message === requestMessages[index])
    ) {
      return fetchImpl(input, init)
    }

    return stringifyJson({ ...parsed.value, messages }).match(
      (serializedBody) => fetchImpl(input, { ...init, body: serializedBody }),
      () => fetchImpl(input, init),
    )
  }
