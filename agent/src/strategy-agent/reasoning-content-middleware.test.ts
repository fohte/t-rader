import { BaseChatModel } from '@langchain/core/language_models/chat_models'
import { AIMessage, HumanMessage } from '@langchain/core/messages'
import type { ChatResult } from '@langchain/core/outputs'
import { createAgent } from 'langchain'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { logger } from '#logger'
import {
  type ChatOpenAIFetch,
  createReasoningContentAwareFetch,
  reasoningContentMiddleware,
} from '#strategy-agent/reasoning-content-middleware'

class RawBodyChatModel extends BaseChatModel {
  constructor(
    private readonly fetchImpl: ChatOpenAIFetch,
    private readonly body: string,
  ) {
    super({})
  }

  override bindTools(
    ...args: Parameters<NonNullable<BaseChatModel['bindTools']>>
  ): ReturnType<NonNullable<BaseChatModel['bindTools']>> {
    void args
    return this
  }

  override _llmType(): string {
    return 'fake'
  }

  override async _generate(): Promise<ChatResult> {
    await this.fetchImpl('http://localhost', { body: this.body })
    return {
      generations: [{ text: 'done', message: new AIMessage('done') }],
    }
  }
}

const runRawBodyRequest = async (body: string, reasoningContent: unknown) => {
  const forwardedBodies: string[] = []
  const warnSpy = vi.spyOn(logger, 'warn').mockImplementation(() => undefined)
  const fetchImpl: ChatOpenAIFetch = (_input, init) => {
    const forwardedBody = init?.body
    forwardedBodies.push(typeof forwardedBody === 'string' ? forwardedBody : '')
    return Promise.resolve(new Response(''))
  }
  const model = new RawBodyChatModel(
    createReasoningContentAwareFetch(fetchImpl),
    body,
  )
  const agent = createAgent({
    model,
    middleware: [reasoningContentMiddleware],
  })

  await agent.invoke({
    messages: [
      new AIMessage({
        content: 'prior response',
        additional_kwargs: { reasoning_content: reasoningContent },
      }),
      new HumanMessage('continue'),
    ],
  })

  return {
    forwardedBodies,
    warnings: warnSpy.mock.calls.map(([fields, message]) => ({
      fields,
      message,
    })),
  }
}

afterEach(() => vi.restoreAllMocks())

describe('reasoningContentMiddleware', () => {
  it.each([
    { name: 'null reasoning content', reasoningContent: null },
    { name: 'empty reasoning content', reasoningContent: '' },
  ])('leaves the request unchanged for $name', async ({ reasoningContent }) => {
    const body = JSON.stringify({
      messages: [{ role: 'assistant', content: 'prior response' }],
    })

    const actual = await runRawBodyRequest(body, reasoningContent)

    expect(actual).toEqual({ forwardedBodies: [body], warnings: [] })
  })

  it.each([
    {
      name: 'a non-JSON body',
      body: 'not-json',
      warnings: [],
    },
    {
      name: 'a body whose messages are not an array',
      body: JSON.stringify({ messages: { role: 'assistant' } }),
      warnings: [],
    },
    {
      name: 'a body with a different assistant count',
      body: JSON.stringify({
        messages: [
          { role: 'assistant', content: 'first' },
          { role: 'assistant', content: 'second' },
        ],
      }),
      warnings: [
        {
          fields: {
            assistantMessageCount: 2,
            reasoningContentCount: 1,
          },
          message:
            'assistant message count differs from reasoning content history',
        },
      ],
    },
  ])('passes through $name', async ({ body, warnings }) => {
    const actual = await runRawBodyRequest(body, 'synthetic reasoning')

    expect(actual).toEqual({ forwardedBodies: [body], warnings })
  })
})
