import { BaseChatModel } from '@langchain/core/language_models/chat_models'
import { AIMessage, HumanMessage } from '@langchain/core/messages'
import type { ChatResult } from '@langchain/core/outputs'
import { createAgent } from 'langchain'
import { describe, expect, it } from 'vitest'

import { AbortedModelCallError } from '#strategy-agent/aborting-model-call-middleware'
import { createModelCallRetryMiddleware } from '#strategy-agent/model-call-retry-middleware'

class ScriptedChatModel extends BaseChatModel {
  attempts = 0

  constructor(private readonly failures: readonly Error[]) {
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
    const failure = this.failures[this.attempts]
    this.attempts += 1
    if (failure !== undefined) return Promise.reject(failure)

    return Promise.resolve({
      generations: [{ text: 'done', message: new AIMessage('done') }],
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

describe('createModelCallRetryMiddleware', () => {
  it('retries a call-duration abort once for each model call', async () => {
    const model = new ScriptedChatModel(
      Array.from(
        { length: 4 },
        () => new AbortedModelCallError('duration expired', 'call-duration'),
      ),
    )
    expect(await runAgent(model, 2)).toEqual({
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

    expect(await runAgent(model)).toEqual({
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
