import { BaseChatModel } from '@langchain/core/language_models/chat_models'
import {
  AIMessage,
  AIMessageChunk,
  HumanMessage,
} from '@langchain/core/messages'
import type { ChatResult } from '@langchain/core/outputs'
import { createAgent } from 'langchain'
import { describe, expect, it, vi } from 'vitest'

import { AbortedModelCallError } from '#strategy-agent/aborting-model-call-middleware'
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
  validateMalformedToolCalls = false,
) =>
  createAgent({
    model,
    tools: [],
    middleware: [
      createModelCallRetryMiddleware(),
      ...(validateMalformedToolCalls
        ? [malformedToolCallResponseMiddleware]
        : []),
    ],
  })

const invokeOutcome = (agent: ReturnType<typeof buildAgent>) =>
  agent.invoke({ messages: [new HumanMessage('sample request')] }).then(
    () => 'resolved' as const,
    () => 'rejected' as const,
  )

const runAgent = async (
  model: ScriptedChatModel,
  invocations = 1,
  validateMalformedToolCalls = false,
) => {
  const agent = buildAgent(model, validateMalformedToolCalls)
  const outcomes: ('resolved' | 'rejected')[] = []

  for (let invocation = 0; invocation < invocations; invocation += 1) {
    outcomes.push(await invokeOutcome(agent))
  }

  return { outcomes, attempts: model.attempts }
}

const runAgentWithFakeTimers = async (
  model: ScriptedChatModel,
  invocations = 1,
  validateMalformedToolCalls = false,
) => {
  vi.useFakeTimers()
  try {
    const result = runAgent(model, invocations, validateMalformedToolCalls)
    await vi.runAllTimersAsync()
    return await result
  } finally {
    vi.useRealTimers()
  }
}

describe('createModelCallRetryMiddleware', () => {
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
      malformedResponse: new AIMessage('<tool_call><function=demo_tool>'),
    },
  ])(
    'retries $name before accepting a valid response',
    async ({ malformedResponse }) => {
      const model = new ScriptedChatModel([
        malformedResponse,
        new AIMessage('done'),
      ])

      expect(await runAgentWithFakeTimers(model, 1, true)).toEqual({
        outcomes: ['resolved'],
        attempts: 2,
      })
    },
  )

  it('fails after three malformed tool call responses', async () => {
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

    expect(await runAgentWithFakeTimers(model, 1, true)).toEqual({
      outcomes: ['rejected'],
      attempts: 3,
    })
  })

  it('passes a valid response without retrying', async () => {
    const model = new ScriptedChatModel([new AIMessage('done')])

    expect(await runAgentWithFakeTimers(model, 1, true)).toEqual({
      outcomes: ['resolved'],
      attempts: 1,
    })
  })

  it('retries a call-duration abort once for each model call', async () => {
    const model = new ScriptedChatModel(
      Array.from(
        { length: 4 },
        () => new AbortedModelCallError('duration expired', 'call-duration'),
      ),
    )
    expect(await runAgentWithFakeTimers(model, 2)).toEqual({
      outcomes: ['rejected', 'rejected'],
      attempts: 4,
    })
  })

  it('keeps two retries for other retryable errors', async () => {
    const serverError = Object.assign(new Error('temporary server failure'), {
      status: 503,
    })
    const timeoutError = new Error('temporary timeout')
    timeoutError.name = 'TimeoutError'
    const model = new ScriptedChatModel([serverError, timeoutError])

    expect(await runAgentWithFakeTimers(model)).toEqual({
      outcomes: ['resolved'],
      attempts: 3,
    })
  })

  it.each([
    {
      name: 'deadline aborts',
      error: new AbortedModelCallError('deadline elapsed', 'deadline'),
    },
    {
      name: 'phase timeouts',
      error: new AbortedModelCallError(
        'phase timeout elapsed',
        'phase-timeout',
      ),
    },
    {
      name: 'usage limits',
      error: Object.assign(new Error('usage limit reached'), {
        rateLimitType: 'stop',
      }),
    },
  ])('does not retry $name', async ({ error }) => {
    const model = new ScriptedChatModel([error])
    expect(await runAgent(model)).toEqual({
      outcomes: ['rejected'],
      attempts: 1,
    })
  })
})
