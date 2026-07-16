import type { Message, Task } from '@a2a-js/sdk'
import type { AgentExecutionEvent, ExecutionEventBus } from '@a2a-js/sdk/server'
import { RequestContext } from '@a2a-js/sdk/server'
import { describe, expect, it } from 'vitest'

import type { TraderAgentExecutorDeps } from '@/a2a/executor'
import { extractStrategyId, TraderAgentExecutor } from '@/a2a/executor'
import type { StrategyAgentResult } from '@/strategy-agent/strategy-agent'

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

const defaultStrategyAgentResult: StrategyAgentResult = {
  status: 'completed',
  message: 'strategy agent result text',
}

describe('TraderAgentExecutor', () => {
  const buildExecutor = (
    runStrategyAgent: TraderAgentExecutorDeps['runStrategyAgent'] = () =>
      Promise.resolve(defaultStrategyAgentResult),
  ): TraderAgentExecutor =>
    new TraderAgentExecutor({
      taskStore: { load: () => Promise.resolve(undefined) },
      runStrategyAgent,
    })

  it('rejects the task when strategy_id is missing', async () => {
    const executor = buildExecutor()
    const eventBus = new FakeEventBus()
    const userMessage = buildUserMessage()
    const requestContext = new RequestContext(userMessage, 'task-1', 'ctx-1')

    await executor.execute(requestContext, eventBus)

    expect(eventBus.finishedCalled).toBe(true)
    expect(
      eventBus.events.map((e) =>
        'kind' in e && e.kind === 'task'
          ? { kind: 'task', state: e.status.state }
          : {
              kind: (e as { kind: string }).kind,
              // eslint-disable-next-line @typescript-eslint/no-unsafe-type-assertion -- `final` only exists on some AgentExecutionEvent variants; test only inspects it when present
              final: (e as { final?: boolean }).final,
            },
      ),
    ).toEqual([
      { kind: 'task', state: 'rejected' },
      { kind: 'status-update', final: true },
    ])
  })

  it('rejects the task when strategy_id is not a valid UUID', async () => {
    const executor = buildExecutor()
    const eventBus = new FakeEventBus()
    const userMessage = buildUserMessage({ strategy_id: 'not-a-uuid' })
    const requestContext = new RequestContext(userMessage, 'task-2', 'ctx-2')

    await executor.execute(requestContext, eventBus)

    // eslint-disable-next-line @typescript-eslint/no-unsafe-type-assertion -- test only reads the shared `status.state` field common to every AgentExecutionEvent variant
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

    // eslint-disable-next-line @typescript-eslint/no-unsafe-type-assertion -- test only reads the shared `status` field common to every AgentExecutionEvent variant
    const [task, working, completed] = eventBus.events as [
      Task,
      { status: { state: string } },
      { status: { state: string; message?: Message } },
    ]
    expect.soft(eventBus.finishedCalled).toBe(true)
    expect.soft(task.status.state).toBe('submitted')
    expect.soft(working.status.state).toBe('working')
    expect.soft(completed.status.state).toBe('completed')
    expect.soft(completed.status.message?.parts[0]).toEqual({
      kind: 'text',
      text: 'strategy agent result text',
    })
  })

  it('maps a failed strategy agent result to a failed task with error_kind metadata', async () => {
    const executor = buildExecutor(() =>
      Promise.resolve({
        status: 'failed',
        message: 'usage limit reached',
        errorKind: 'usage_limit',
      }),
    )
    const eventBus = new FakeEventBus()
    const userMessage = buildUserMessage({
      strategy_id: '11111111-1111-1111-1111-111111111111',
    })
    const requestContext = new RequestContext(userMessage, 'task-5', 'ctx-5')

    await executor.execute(requestContext, eventBus)

    // eslint-disable-next-line @typescript-eslint/no-unsafe-type-assertion -- test only reads the shared `status` field common to every AgentExecutionEvent variant
    const completed = eventBus.events[2] as {
      status: { timestamp: string; message?: Message }
    }
    const { timestamp } = completed.status
    const { messageId } = completed.status.message ?? {}

    expect(eventBus.finishedCalled).toBe(true)
    expect(completed).toEqual({
      kind: 'status-update',
      taskId: 'task-5',
      contextId: 'ctx-5',
      final: true,
      status: {
        state: 'failed',
        timestamp,
        message: {
          kind: 'message',
          role: 'agent',
          messageId,
          taskId: 'task-5',
          contextId: 'ctx-5',
          parts: [{ kind: 'text', text: 'usage limit reached' }],
          metadata: { error_kind: 'usage_limit' },
        },
      },
    })
  })

  it('publishes a failed status-update and still calls finished() when runStrategyAgent rejects unexpectedly', async () => {
    const executor = buildExecutor(() =>
      Promise.reject(new Error('mcp client construction blew up')),
    )
    const eventBus = new FakeEventBus()
    const userMessage = buildUserMessage({
      strategy_id: '11111111-1111-1111-1111-111111111111',
    })
    const requestContext = new RequestContext(userMessage, 'task-6', 'ctx-6')

    await executor.execute(requestContext, eventBus)

    // eslint-disable-next-line @typescript-eslint/no-unsafe-type-assertion -- test only reads the shared `status` field common to every AgentExecutionEvent variant
    const completed = eventBus.events[2] as {
      status: { timestamp: string; message?: Message }
    }
    const { timestamp } = completed.status
    const { messageId } = completed.status.message ?? {}

    expect(eventBus.finishedCalled).toBe(true)
    expect(completed).toEqual({
      kind: 'status-update',
      taskId: 'task-6',
      contextId: 'ctx-6',
      final: true,
      status: {
        state: 'failed',
        timestamp,
        message: {
          kind: 'message',
          role: 'agent',
          messageId,
          taskId: 'task-6',
          contextId: 'ctx-6',
          parts: [{ kind: 'text', text: 'mcp client construction blew up' }],
          metadata: { error_kind: 'agent_error' },
        },
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
      runStrategyAgent: () => Promise.resolve(defaultStrategyAgentResult),
    })

    await executor.cancelTask('task-4', eventBus)

    // eslint-disable-next-line @typescript-eslint/no-unsafe-type-assertion -- test only reads the shared `status.timestamp` field common to every AgentExecutionEvent variant
    const statusUpdateEvent = eventBus.events[0] as {
      status: { timestamp: string }
    }
    const { timestamp } = statusUpdateEvent.status
    expect(Number.isNaN(new Date(timestamp).getTime())).toBe(false)

    expect(eventBus.finishedCalled).toBe(true)
    expect(eventBus.events[0]).toEqual({
      kind: 'status-update',
      taskId: 'task-4',
      contextId: 'ctx-4',
      final: true,
      status: { state: 'canceled', timestamp },
    })
  })
})
