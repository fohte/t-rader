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

const buildAgent = (model: ScriptedChatModel) =>
  createAgent({
    model,
    tools: [],
    middleware: [createModelCallRetryMiddleware()],
  })

const invokeOutcome = (agent: ReturnType<typeof buildAgent>) =>
  agent.invoke({ messages: [new HumanMessage('sample request')] }).then(
    () => 'resolved' as const,
    () => 'rejected' as const,
  )

const runAgent = async (model: ScriptedChatModel, invocations = 1) => {
  const agent = buildAgent(model)
  const outcomes: ('resolved' | 'rejected')[] = []

  for (let invocation = 0; invocation < invocations; invocation += 1) {
    outcomes.push(await invokeOutcome(agent))
  }

  return { outcomes, attempts: model.attempts }
}

const runAgentWithFakeTimers = async (
  model: ScriptedChatModel,
  invocations = 1,
) => {
  vi.useFakeTimers()
  try {
    const result = runAgent(model, invocations)
    await vi.runAllTimersAsync()
    return await result
  } finally {
    vi.useRealTimers()
  }
}

describe('createModelCallRetryMiddleware', () => {
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

  it('uses a new consumed reservation for each actual retry', async () => {
    const model = new ScriptedChatModel([
      Object.assign(new Error('temporary server failure'), { status: 503 }),
      new AIMessage('done'),
    ])
    const reserved: string[] = []
    const consumed: string[] = []
    const agent = createAgent({
      model,
      tools: [],
      middleware: [
        createModelCallRetryMiddleware(
          (callId) => {
            reserved.push(callId)
            return Promise.resolve()
          },
          (callId, state) => {
            if (state === 'consumed') consumed.push(callId)
            return Promise.resolve()
          },
        ),
      ],
    })

    const outcome = await invokeOutcome(agent)

    const actual = JSON.stringify({
      outcome,
      attempts: model.attempts,
      reservations: reserved.length,
      retryUsesAnotherCallId: new Set(reserved).size === 2,
      eachAttemptWasConsumed:
        consumed.length === 2 &&
        consumed.every((callId) => reserved.includes(callId)),
    })
    expect(actual).toBe(
      '{"outcome":"resolved","attempts":2,"reservations":2,"retryUsesAnotherCallId":true,"eachAttemptWasConsumed":true}',
    )
  })

  it('does not call the model when its reservation request fails', async () => {
    const model = new ScriptedChatModel([new AIMessage('done')])
    let reservationAttempts = 0
    let transitions = 0
    const agent = createAgent({
      model,
      tools: [],
      middleware: [
        createModelCallRetryMiddleware(
          () => {
            reservationAttempts += 1
            return Promise.reject(new TypeError('reservation transport failed'))
          },
          () => {
            transitions += 1
            return Promise.resolve()
          },
        ),
      ],
    })

    const outcome = await invokeOutcome(agent)

    const actual = JSON.stringify({
      outcome,
      attempts: model.attempts,
      reservationAttempts,
      transitions,
    })
    expect(actual).toBe(
      '{"outcome":"rejected","attempts":0,"reservationAttempts":1,"transitions":0}',
    )
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
