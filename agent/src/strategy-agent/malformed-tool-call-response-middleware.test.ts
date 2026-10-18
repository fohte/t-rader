import { BaseChatModel } from '@langchain/core/language_models/chat_models'
import {
  AIMessage,
  AIMessageChunk,
  HumanMessage,
} from '@langchain/core/messages'
import type { ChatResult } from '@langchain/core/outputs'
import { DynamicStructuredTool } from '@langchain/core/tools'
import { createAgent } from 'langchain'
import { describe, expect, it, vi } from 'vitest'
import { z } from 'zod'

import { malformedToolCallResponseMiddleware } from '#strategy-agent/malformed-tool-call-response-middleware'
import { createModelCallRetryMiddleware } from '#strategy-agent/model-call-retry-middleware'

class ScriptedChatModel extends BaseChatModel {
  attempts = 0

  constructor(
    private readonly responses: readonly (Error | AIMessage | AIMessageChunk)[],
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
    return 'scripted'
  }

  override _generate(): Promise<ChatResult> {
    const response = this.responses[this.attempts]
    this.attempts += 1
    if (response instanceof Error) return Promise.reject(response)

    return Promise.resolve({
      generations: [
        { text: 'done', message: response ?? new AIMessage('done') },
      ],
    })
  }
}

const buildAgent = (
  model: ScriptedChatModel,
  tools: readonly DynamicStructuredTool[] = [],
) =>
  createAgent({
    model,
    tools: [...tools],
    middleware: [
      createModelCallRetryMiddleware(),
      malformedToolCallResponseMiddleware,
    ],
  })

const runAgent = async (
  model: ScriptedChatModel,
  options: {
    readonly tools?: readonly DynamicStructuredTool[]
    readonly toolArguments?: { field: string }[]
  } = {},
) => {
  const agent = buildAgent(model, options.tools)
  vi.useFakeTimers()
  try {
    const result = agent
      .invoke({ messages: [new HumanMessage('sample request')] })
      .then(
        () => 'resolved' as const,
        () => 'rejected' as const,
      )
    await vi.runAllTimersAsync()
    return {
      outcome: await result,
      attempts: model.attempts,
      toolArguments: options.toolArguments ?? [],
    }
  } finally {
    vi.useRealTimers()
  }
}

describe('malformedToolCallResponseMiddleware', () => {
  it.each([
    {
      name: 'raw tool arguments that are not JSON',
      malformedResponse: new AIMessageChunk({
        content: '',
        tool_call_chunks: [
          {
            id: 'call_demo',
            name: 'demo_tool',
            args: '{"field": ',
            index: 0,
          },
        ],
      }),
    },
    {
      name: 'invalid tool calls',
      malformedResponse: new AIMessage({
        content: '',
        invalid_tool_calls: [
          {
            id: 'call_demo',
            name: 'demo_tool',
            args: 'not-json',
            error: 'Malformed args.',
          },
        ],
      }),
    },
    {
      name: 'tool call markup in message content',
      malformedResponse: new AIMessage(
        '<tool_call><function=demo_tool><parameter=field>value</parameter>',
      ),
    },
  ])(
    'retries $name before accepting a valid response',
    async ({ malformedResponse }) => {
      const model = new ScriptedChatModel([
        malformedResponse,
        new AIMessage('done'),
      ])

      expect(await runAgent(model)).toEqual({
        outcome: 'resolved',
        attempts: 2,
        toolArguments: [],
      })
    },
  )

  it('fails after three malformed responses', async () => {
    const malformedResponse = new AIMessageChunk({
      content: '',
      tool_call_chunks: [
        {
          id: 'call_demo',
          name: 'demo_tool',
          args: '{"field": ',
          index: 0,
        },
      ],
    })
    const model = new ScriptedChatModel([
      malformedResponse,
      malformedResponse,
      malformedResponse,
    ])

    expect(await runAgent(model)).toEqual({
      outcome: 'rejected',
      attempts: 3,
      toolArguments: [],
    })
  })

  it('passes a valid response without retrying', async () => {
    const model = new ScriptedChatModel([new AIMessage('done')])

    expect(await runAgent(model)).toEqual({
      outcome: 'resolved',
      attempts: 1,
      toolArguments: [],
    })
  })

  it('executes a tool call with valid streamed arguments', async () => {
    const toolArguments: { field: string }[] = []
    const tool = new DynamicStructuredTool({
      name: 'demo_tool',
      description: 'Returns a demo result',
      schema: z.object({ field: z.string() }),
      func: (input) => {
        toolArguments.push(input)
        return Promise.resolve('tool result')
      },
    })
    const model = new ScriptedChatModel([
      new AIMessageChunk({
        content: '',
        tool_call_chunks: [
          {
            id: 'call_demo',
            name: 'demo_tool',
            args: '{"field":"valid"}',
            index: 0,
          },
        ],
      }),
      new AIMessage('done'),
    ])

    expect(await runAgent(model, { tools: [tool], toolArguments })).toEqual({
      outcome: 'resolved',
      attempts: 2,
      toolArguments: [{ field: 'valid' }],
    })
  })
})
