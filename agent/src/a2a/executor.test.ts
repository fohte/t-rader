import type { Message, Task } from '@a2a-js/sdk'
import type { AgentExecutionEvent, ExecutionEventBus } from '@a2a-js/sdk/server'
import { RequestContext } from '@a2a-js/sdk/server'
import { describe, expect, it } from 'vitest'

import { extractStrategyId, TraderAgentExecutor } from '@/a2a/executor'

class FakeEventBus implements ExecutionEventBus {
  public readonly events: AgentExecutionEvent[] = []
  public finishedCalled = false

  publish(event: AgentExecutionEvent): void {
    this.events.push(event)
  }
  on(): this {
    return this
  }
  off(): this {
    return this
  }
  once(): this {
    return this
  }
  removeAllListeners(): this {
    return this
  }
  finished(): void {
    this.finishedCalled = true
  }
}

const buildUserMessage = (metadata?: Record<string, unknown>): Message => ({
  kind: 'message',
  role: 'user',
  messageId: 'm1',
  parts: [{ kind: 'text', text: 'do the thing' }],
  ...(metadata !== undefined ? { metadata } : {}),
})

describe('extractStrategyId', () => {
  it('reads strategy_id from message metadata', () => {
    expect(
      extractStrategyId(
        buildUserMessage({
          strategy_id: '11111111-1111-1111-1111-111111111111',
        }),
      ),
    ).toBe('11111111-1111-1111-1111-111111111111')
  })

  it('returns undefined when metadata is absent', () => {
    expect(extractStrategyId(buildUserMessage())).toBeUndefined()
  })

  it('returns undefined when strategy_id is not a string', () => {
    expect(
      extractStrategyId(buildUserMessage({ strategy_id: 123 })),
    ).toBeUndefined()
  })
})

describe('TraderAgentExecutor', () => {
  const buildExecutor = (): TraderAgentExecutor =>
    new TraderAgentExecutor({
      taskStore: { load: () => Promise.resolve(undefined) },
    })

  it('rejects the task when strategy_id is missing', async () => {
    const executor = buildExecutor()
    const eventBus = new FakeEventBus()
    const userMessage = buildUserMessage()
    const requestContext = new RequestContext(userMessage, 'task-1', 'ctx-1')

    await executor.execute(requestContext, eventBus)

    expect({
      finished: eventBus.finishedCalled,
      events: eventBus.events.map((e) =>
        'kind' in e && e.kind === 'task'
          ? { kind: 'task', state: (e as Task).status.state }
          : {
              kind: (e as { kind: string }).kind,
              final: (e as { final?: boolean }).final,
            },
      ),
    }).toEqual({
      finished: true,
      events: [
        { kind: 'task', state: 'rejected' },
        { kind: 'status-update', final: true },
      ],
    })
  })

  it('rejects the task when strategy_id is not a valid UUID', async () => {
    const executor = buildExecutor()
    const eventBus = new FakeEventBus()
    const userMessage = buildUserMessage({ strategy_id: 'not-a-uuid' })
    const requestContext = new RequestContext(userMessage, 'task-2', 'ctx-2')

    await executor.execute(requestContext, eventBus)

    const finalEvent = eventBus.events[1] as { status: { state: string } }
    expect(finalEvent.status.state).toBe('rejected')
  })

  it('drives a valid strategy_id through submitted -> working -> completed', async () => {
    const executor = buildExecutor()
    const eventBus = new FakeEventBus()
    const userMessage = buildUserMessage({
      strategy_id: '11111111-1111-1111-1111-111111111111',
    })
    const requestContext = new RequestContext(userMessage, 'task-3', 'ctx-3')

    await executor.execute(requestContext, eventBus)

    const [task, working, completed] = eventBus.events as [
      Task,
      { status: { state: string } },
      { status: { state: string; message?: Message } },
    ]
    expect({
      finished: eventBus.finishedCalled,
      taskState: task.status.state,
      workingState: working.status.state,
      completedState: completed.status.state,
      completedMessageText: completed.status.message?.parts[0],
    }).toEqual({
      finished: true,
      taskState: 'submitted',
      workingState: 'working',
      completedState: 'completed',
      completedMessageText: {
        kind: 'text',
        text: 'strategy agent execution is not implemented yet',
      },
    })
  })

  it('publishes a canceled status-update using the stored task contextId', async () => {
    const eventBus = new FakeEventBus()
    const executor = new TraderAgentExecutor({
      taskStore: {
        load: () =>
          Promise.resolve({
            id: 'task-4',
            contextId: 'ctx-4',
            kind: 'task',
            status: { state: 'working', timestamp: '2026-01-01T00:00:00.000Z' },
          } as Task),
      },
    })

    await executor.cancelTask('task-4', eventBus)

    expect({
      finished: eventBus.finishedCalled,
      event: eventBus.events[0],
    }).toEqual({
      finished: true,
      event: {
        kind: 'status-update',
        taskId: 'task-4',
        contextId: 'ctx-4',
        final: true,
        status: {
          state: 'canceled',
          timestamp: (eventBus.events[0] as { status: { timestamp: string } })
            .status.timestamp,
        },
      },
    })
  })
})
