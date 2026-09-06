import type { Message, Task, TaskStatusUpdateEvent } from '@a2a-js/sdk'
import type { AgentExecutionEvent, ExecutionEventBus } from '@a2a-js/sdk/server'
import { RequestContext } from '@a2a-js/sdk/server'
import { errAsync, okAsync } from 'neverthrow'
import { describe, expect, it, vi } from 'vitest'

import type { TraderAgentExecutorDeps } from '#a2a/executor'
import {
  extractStrategyId,
  HEARTBEAT_INTERVAL_MS,
  TraderAgentExecutor,
} from '#a2a/executor'
import type { StrategyTaskStep } from '#strategy-agent/agent-graph/step'
import type { StrategyAgentResult } from '#strategy-agent/strategy-agent'
import { StrategyCandidatesFetchError } from '#strategy-resolution/mgmt-mcp-client'
import type { StrategyCandidate } from '#strategy-resolution/resolve-strategy'

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

const buildUserMessage = (
  metadata?: Record<string, unknown>,
  text = 'do the thing',
): Message => ({
  kind: 'message',
  role: 'user',
  messageId: 'm1',
  parts: [{ kind: 'text', text }],
  ...(metadata !== undefined ? { metadata } : {}),
})

const buildAgentMessage = (text: string): Message => ({
  kind: 'message',
  role: 'agent',
  messageId: 'agent-m1',
  parts: [{ kind: 'text', text }],
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

const CANDIDATES: readonly StrategyCandidate[] = [
  { strategyId: '11111111-1111-1111-1111-111111111111', name: '長期投資' },
  { strategyId: '22222222-2222-2222-2222-222222222222', name: '中期投資' },
]

type StatusEvent = {
  status: { state: string; timestamp: string; message?: Message }
}

describe('TraderAgentExecutor', () => {
  const buildExecutor = (
    overrides: Partial<TraderAgentExecutorDeps> = {},
  ): TraderAgentExecutor =>
    new TraderAgentExecutor({
      taskStore: { load: () => Promise.resolve(undefined) },
      runStrategyAgent: () => Promise.resolve(defaultStrategyAgentResult),
      fetchStrategyCandidates: () => okAsync(CANDIDATES),
      ...overrides,
    })

  it('rejects the task when strategy_id metadata is present but not a valid UUID', async () => {
    const executor = buildExecutor()
    const eventBus = new FakeEventBus()
    const userMessage = buildUserMessage({ strategy_id: 'not-a-uuid' })
    const requestContext = new RequestContext(userMessage, 'task-2', 'ctx-2')

    await executor.execute(requestContext, eventBus)

    // eslint-disable-next-line @typescript-eslint/no-unsafe-type-assertion -- test only reads the shared `status` field common to every AgentExecutionEvent variant
    const [, rejected] = eventBus.events as [Task, StatusEvent]
    expect(eventBus.finishedCalled).toBe(true)
    expect(rejected.status.state).toBe('rejected')
  })

  it('rejects a resumed task without republishing (and clobbering) its stored history', async () => {
    const executor = buildExecutor()
    const eventBus = new FakeEventBus()
    const priorMessage = buildUserMessage(undefined, 'earlier turn')
    const followUpMessage = buildUserMessage({ strategy_id: 'not-a-uuid' })
    const existingTask: Task = {
      id: 'task-2b',
      contextId: 'ctx-2b',
      kind: 'task',
      history: [priorMessage, followUpMessage],
      status: {
        state: 'input-required',
        timestamp: '2026-01-01T00:00:00.000Z',
      },
    }
    const requestContext = new RequestContext(
      followUpMessage,
      'task-2b',
      'ctx-2b',
      existingTask,
    )

    await executor.execute(requestContext, eventBus)

    expect(eventBus.finishedCalled).toBe(true)
    expect(eventBus.events).toHaveLength(1)
    expect(eventBus.events.every((e) => e.kind === 'status-update')).toBe(true)
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
      StatusEvent,
      StatusEvent,
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

  it('publishes an artifact-update event when runStrategyAgent reports step progress', async () => {
    const executor = buildExecutor({
      runStrategyAgent: (
        _strategyId,
        _taskId,
        _userMessage,
        onStepsChanged,
      ) => {
        onStepsChanged?.([
          {
            phaseKey: 'plan',
            label: '調査計画',
            model: 'claude-opus-4',
            status: 'running',
            startedAt: '2026-01-01T00:00:00.000Z',
            traceId: 'trace-1',
            spanId: 'span-1',
          },
        ])
        return Promise.resolve(defaultStrategyAgentResult)
      },
    })
    const eventBus = new FakeEventBus()
    const userMessage = buildUserMessage({
      strategy_id: '11111111-1111-1111-1111-111111111111',
    })
    const requestContext = new RequestContext(userMessage, 'task-13', 'ctx-13')

    await executor.execute(requestContext, eventBus)

    const artifactUpdate = eventBus.events.find(
      (e) => e.kind === 'artifact-update',
    )
    expect(artifactUpdate).toEqual({
      kind: 'artifact-update',
      taskId: 'task-13',
      contextId: 'ctx-13',
      artifact: {
        artifactId: 'agent-graph-steps',
        name: 'agent-graph-steps',
        parts: [
          {
            kind: 'data',
            data: {
              steps: [
                {
                  phase_key: 'plan',
                  label: '調査計画',
                  model: 'claude-opus-4',
                  status: 'running',
                  started_at: '2026-01-01T00:00:00.000Z',
                  trace_id: 'trace-1',
                  span_id: 'span-1',
                },
              ],
            },
          },
        ],
      },
    })
  })

  describe('heartbeat status-update on step progress', () => {
    const buildStep = (
      overrides: Partial<StrategyTaskStep> &
        Pick<StrategyTaskStep, 'status' | 'startedAt'>,
    ): StrategyTaskStep => ({
      phaseKey: 'discover',
      label: '調査',
      model: 'claude-opus-4',
      traceId: 'trace-1',
      spanId: 'span-1',
      ...overrides,
    })

    const runWithSteps = async (
      steps: readonly StrategyTaskStep[],
    ): Promise<TaskStatusUpdateEvent[]> => {
      const executor = buildExecutor({
        runStrategyAgent: (
          _strategyId,
          _taskId,
          _userMessage,
          onStepsChanged,
        ) => {
          onStepsChanged?.(steps)
          return Promise.resolve(defaultStrategyAgentResult)
        },
      })
      const eventBus = new FakeEventBus()
      const userMessage = buildUserMessage({
        strategy_id: '11111111-1111-1111-1111-111111111111',
      })
      const requestContext = new RequestContext(
        userMessage,
        'task-14',
        'ctx-14',
      )
      await executor.execute(requestContext, eventBus)
      // The very first working status-update (published before
      // runStrategyAgent even starts) carries no message; only the
      // step-progress heartbeats do.
      return eventBus.events.filter(
        (e): e is TaskStatusUpdateEvent =>
          e.kind === 'status-update' &&
          e.status.state === 'working' &&
          e.status.message !== undefined,
      )
    }

    it.each([
      {
        name: 'running',
        status: 'running' as const,
        text: 'フェーズ「調査」が実行中',
      },
      {
        name: 'completed',
        status: 'completed' as const,
        text: 'フェーズ「調査」が完了',
      },
      {
        name: 'failed',
        status: 'failed' as const,
        text: 'フェーズ「調査」が失敗',
      },
    ])(
      'renders $name step status in the progress text',
      async ({ status, text }) => {
        const heartbeats = await runWithSteps([
          buildStep({ status, startedAt: '2026-01-01T00:00:00.000Z' }),
        ])
        const [heartbeat] = heartbeats
        expect(heartbeat?.status.message?.parts[0]).toEqual({
          kind: 'text',
          text,
        })
      },
    )

    it('reports the step that changed most recently, not the array tail, when an earlier-started for_each item finishes last', async () => {
      const heartbeats = await runWithSteps([
        buildStep({
          phaseKey: 'research',
          label: '深掘り',
          status: 'completed',
          item: { symbol: '7203' },
          itemLabel: '7203',
          startedAt: '2026-01-01T00:00:00.000Z',
          finishedAt: '2026-01-01T00:02:00.000Z',
        }),
        buildStep({
          phaseKey: 'research',
          label: '深掘り',
          status: 'running',
          item: { symbol: '6758' },
          itemLabel: '6758',
          startedAt: '2026-01-01T00:00:30.000Z',
        }),
      ])
      const [heartbeat] = heartbeats
      expect(heartbeat?.status.message?.parts[0]).toEqual({
        kind: 'text',
        text: 'フェーズ「深掘り」(1/2): 7203 が完了',
      })
    })

    it('reuses the same heartbeat messageId across step changes so history dedup keeps only the first entry', async () => {
      const executor = buildExecutor({
        runStrategyAgent: (
          _strategyId,
          _taskId,
          _userMessage,
          onStepsChanged,
        ) => {
          onStepsChanged?.([
            buildStep({
              status: 'running',
              startedAt: '2026-01-01T00:00:00.000Z',
            }),
          ])
          onStepsChanged?.([
            buildStep({
              status: 'completed',
              startedAt: '2026-01-01T00:00:00.000Z',
              finishedAt: '2026-01-01T00:01:00.000Z',
            }),
          ])
          return Promise.resolve(defaultStrategyAgentResult)
        },
      })
      const eventBus = new FakeEventBus()
      const userMessage = buildUserMessage({
        strategy_id: '11111111-1111-1111-1111-111111111111',
      })
      const requestContext = new RequestContext(
        userMessage,
        'task-15',
        'ctx-15',
      )

      await executor.execute(requestContext, eventBus)

      const heartbeats = eventBus.events.filter(
        (e): e is TaskStatusUpdateEvent =>
          e.kind === 'status-update' && e.status.state === 'working',
      )
      const [, firstHeartbeat, secondHeartbeat] = heartbeats
      expect(firstHeartbeat?.status.message?.messageId).toEqual(
        secondHeartbeat?.status.message?.messageId,
      )
    })
  })

  describe('periodic heartbeat while a phase runs without step changes', () => {
    it('republishes the working status-update on every interval tick, and stops once the task settles', async () => {
      vi.useFakeTimers()
      try {
        let resolveAgent: (result: StrategyAgentResult) => void = () => {
          throw new Error('resolveAgent called before assignment')
        }
        const agentPromise = new Promise<StrategyAgentResult>((resolve) => {
          resolveAgent = resolve
        })
        const executor = buildExecutor({
          runStrategyAgent: () => agentPromise,
        })
        const eventBus = new FakeEventBus()
        const userMessage = buildUserMessage({
          strategy_id: '11111111-1111-1111-1111-111111111111',
        })
        const requestContext = new RequestContext(
          userMessage,
          'task-16',
          'ctx-16',
        )
        const workingUpdates = (): TaskStatusUpdateEvent[] =>
          eventBus.events.filter(
            (e): e is TaskStatusUpdateEvent =>
              e.kind === 'status-update' && e.status.state === 'working',
          )

        const executePromise = executor.execute(requestContext, eventBus)

        // 初回の working (メッセージなし) の後、step の変化がなくても
        // heartbeatTimer が interval ごとに working を再送する。
        await vi.advanceTimersByTimeAsync(HEARTBEAT_INTERVAL_MS * 2)
        const [, firstTick, secondTick] = workingUpdates()
        expect(workingUpdates()).toHaveLength(3)
        // Task.history を汚さないため、interval 発火分も同じ messageId を
        // 使い回す (publishSteps 由来の heartbeat と同じ契約)。
        expect(firstTick?.status.message?.messageId).toEqual(
          secondTick?.status.message?.messageId,
        )

        resolveAgent(defaultStrategyAgentResult)
        await executePromise

        // finally で heartbeatTimer を止めているため、決着後は interval が
        // 進んでも working は増えない。
        await vi.advanceTimersByTimeAsync(HEARTBEAT_INTERVAL_MS * 3)
        expect(workingUpdates()).toHaveLength(3)
      } finally {
        vi.useRealTimers()
      }
    })
  })

  it('maps a failed strategy agent result to a failed task with error_kind metadata', async () => {
    const executor = buildExecutor({
      runStrategyAgent: () =>
        Promise.resolve({
          status: 'failed',
          message: 'usage limit reached',
          errorKind: 'usage_limit',
        }),
    })
    const eventBus = new FakeEventBus()
    const userMessage = buildUserMessage({
      strategy_id: '11111111-1111-1111-1111-111111111111',
    })
    const requestContext = new RequestContext(userMessage, 'task-5', 'ctx-5')

    await executor.execute(requestContext, eventBus)

    // eslint-disable-next-line @typescript-eslint/no-unsafe-type-assertion -- test only reads the shared `status` field common to every AgentExecutionEvent variant
    const [, , completed] = eventBus.events as [Task, StatusEvent, StatusEvent]
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
    const executor = buildExecutor({
      runStrategyAgent: () =>
        Promise.reject(new Error('mcp client construction blew up')),
    })
    const eventBus = new FakeEventBus()
    const userMessage = buildUserMessage({
      strategy_id: '11111111-1111-1111-1111-111111111111',
    })
    const requestContext = new RequestContext(userMessage, 'task-6', 'ctx-6')

    await executor.execute(requestContext, eventBus)

    // eslint-disable-next-line @typescript-eslint/no-unsafe-type-assertion -- test only reads the shared `status` field common to every AgentExecutionEvent variant
    const [, , completed] = eventBus.events as [Task, StatusEvent, StatusEvent]
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
      fetchStrategyCandidates: () => okAsync(CANDIDATES),
    })

    await executor.cancelTask('task-4', eventBus)

    // eslint-disable-next-line @typescript-eslint/no-unsafe-type-assertion -- test only reads the shared `status.timestamp` field common to every AgentExecutionEvent variant
    const [statusUpdateEvent] = eventBus.events as unknown as [StatusEvent]
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

  describe('strategy resolution from free text (no strategy_id metadata)', () => {
    it('resolves the strategy uniquely from message content and runs it', async () => {
      const calls: { strategyId: string; taskId: string; text: string }[] = []
      const executor = buildExecutor({
        runStrategyAgent: (strategyId, taskId, userMessage) => {
          calls.push({
            strategyId,
            taskId,
            text: userMessage.parts
              .map((p) => (p.kind === 'text' ? p.text : ''))
              .join('\n'),
          })
          return Promise.resolve(defaultStrategyAgentResult)
        },
      })
      const eventBus = new FakeEventBus()
      const userMessage = buildUserMessage(
        undefined,
        '長期投資でNVDAを分析して',
      )
      const requestContext = new RequestContext(userMessage, 'task-7', 'ctx-7')

      await executor.execute(requestContext, eventBus)

      // eslint-disable-next-line @typescript-eslint/no-unsafe-type-assertion -- test only reads the shared `status` field common to every AgentExecutionEvent variant
      const [task, working, completed] = eventBus.events as [
        Task,
        StatusEvent,
        StatusEvent,
      ]
      expect.soft(eventBus.finishedCalled).toBe(true)
      expect.soft(task.status.state).toBe('submitted')
      expect.soft(working.status.state).toBe('working')
      expect.soft(completed.status.state).toBe('completed')
      expect(calls).toEqual([
        {
          strategyId: '11111111-1111-1111-1111-111111111111',
          taskId: 'task-7',
          text: '長期投資でNVDAを分析して',
        },
      ])
    })

    it('transitions to input-required when multiple strategies match ambiguously', async () => {
      const executor = buildExecutor()
      const eventBus = new FakeEventBus()
      const userMessage = buildUserMessage(
        undefined,
        '投資戦略でNVDAを分析して',
      )
      const requestContext = new RequestContext(userMessage, 'task-8', 'ctx-8')

      await executor.execute(requestContext, eventBus)

      // eslint-disable-next-line @typescript-eslint/no-unsafe-type-assertion -- test only reads the shared `status` field common to every AgentExecutionEvent variant
      const [, , final] = eventBus.events as [Task, StatusEvent, StatusEvent]
      const { timestamp } = final.status
      const { messageId } = final.status.message ?? {}
      expect(eventBus.finishedCalled).toBe(true)
      expect(final.status).toEqual({
        state: 'input-required',
        timestamp,
        message: {
          kind: 'message',
          role: 'agent',
          messageId,
          taskId: 'task-8',
          contextId: 'ctx-8',
          parts: [
            {
              kind: 'text',
              text: '対象の戦略を一意に特定できませんでした。次のうちどれですか: 長期投資, 中期投資',
            },
          ],
        },
      })
    })

    it('transitions to input-required when no strategy matches', async () => {
      const executor = buildExecutor()
      const eventBus = new FakeEventBus()
      const userMessage = buildUserMessage(undefined, 'こんにちは')
      const requestContext = new RequestContext(userMessage, 'task-9', 'ctx-9')

      await executor.execute(requestContext, eventBus)

      // eslint-disable-next-line @typescript-eslint/no-unsafe-type-assertion -- test only reads the shared `status` field common to every AgentExecutionEvent variant
      const [, , final] = eventBus.events as [Task, StatusEvent, StatusEvent]
      const { timestamp } = final.status
      const { messageId } = final.status.message ?? {}
      expect(eventBus.finishedCalled).toBe(true)
      expect(final.status).toEqual({
        state: 'input-required',
        timestamp,
        message: {
          kind: 'message',
          role: 'agent',
          messageId,
          taskId: 'task-9',
          contextId: 'ctx-9',
          parts: [
            {
              kind: 'text',
              text: '対象の戦略が見つかりませんでした。次のいずれかの戦略名を含めて教えてください: 長期投資, 中期投資',
            },
          ],
        },
      })
    })

    it('transitions to input-required with a dedicated message when there are no strategies to choose from', async () => {
      const executor = buildExecutor({
        fetchStrategyCandidates: () => okAsync([]),
      })
      const eventBus = new FakeEventBus()
      const userMessage = buildUserMessage(undefined, 'NVDAを分析して')
      const requestContext = new RequestContext(
        userMessage,
        'task-12',
        'ctx-12',
      )

      await executor.execute(requestContext, eventBus)

      // eslint-disable-next-line @typescript-eslint/no-unsafe-type-assertion -- test only reads the shared `status` field common to every AgentExecutionEvent variant
      const [, , final] = eventBus.events as [Task, StatusEvent, StatusEvent]
      expect(eventBus.finishedCalled).toBe(true)
      expect(final.status.state).toBe('input-required')
      expect(final.status.message?.parts[0]).toEqual({
        kind: 'text',
        text: '利用可能な戦略が登録されていません。',
      })
    })

    // Failing this turn synchronously (rather than leaving the task stuck in
    // working) matters because there's no retry loop above the executor —
    // the only other way the task would ever settle is the watchdog's
    // timeout, minutes later.
    it('fails immediately when fetchStrategyCandidates returns an error', async () => {
      const executor = buildExecutor({
        fetchStrategyCandidates: () =>
          errAsync(new StrategyCandidatesFetchError('mgmt MCP unreachable')),
      })
      const eventBus = new FakeEventBus()
      const userMessage = buildUserMessage(
        undefined,
        '長期投資でNVDAを分析して',
      )
      const requestContext = new RequestContext(
        userMessage,
        'task-10',
        'ctx-10',
      )

      await executor.execute(requestContext, eventBus)

      // eslint-disable-next-line @typescript-eslint/no-unsafe-type-assertion -- test only reads the shared `status` field common to every AgentExecutionEvent variant
      const [, , final] = eventBus.events as [Task, StatusEvent, StatusEvent]
      const { timestamp } = final.status
      const { messageId } = final.status.message ?? {}
      expect(eventBus.finishedCalled).toBe(true)
      expect(final.status).toEqual({
        state: 'failed',
        timestamp,
        message: {
          kind: 'message',
          role: 'agent',
          messageId,
          taskId: 'task-10',
          contextId: 'ctx-10',
          parts: [{ kind: 'text', text: 'mgmt MCP unreachable' }],
          metadata: { error_kind: 'strategy_resolution_error' },
        },
      })
    })

    it('re-resolves from the full message history when a follow-up message resumes an input-required task, without republishing the task history', async () => {
      const calls: { strategyId: string; taskId: string; text: string }[] = []
      const executor = buildExecutor({
        runStrategyAgent: (strategyId, taskId, userMessage) => {
          calls.push({
            strategyId,
            taskId,
            text: userMessage.parts
              .map((p) => (p.kind === 'text' ? p.text : ''))
              .join('\n'),
          })
          return Promise.resolve(defaultStrategyAgentResult)
        },
      })
      const eventBus = new FakeEventBus()

      const firstUserMessage = buildUserMessage(
        undefined,
        '投資戦略でNVDAを分析して',
      )
      const clarifyingQuestion = buildAgentMessage(
        '対象の戦略を一意に特定できませんでした。次のうちどれですか: 長期投資, 中期投資',
      )
      const followUpMessage = buildUserMessage(
        undefined,
        '長期の方でお願いします',
      )
      // Mirrors what DefaultRequestHandler._createRequestContext does before
      // calling execute(): the incoming follow-up message is already
      // appended to the stored task's history.
      const existingTask: Task = {
        id: 'task-11',
        contextId: 'ctx-11',
        kind: 'task',
        history: [firstUserMessage, clarifyingQuestion, followUpMessage],
        status: {
          state: 'input-required',
          timestamp: '2026-01-01T00:00:00.000Z',
        },
      }
      const requestContext = new RequestContext(
        followUpMessage,
        'task-11',
        'ctx-11',
        existingTask,
      )

      await executor.execute(requestContext, eventBus)

      // eslint-disable-next-line @typescript-eslint/no-unsafe-type-assertion -- test only reads the shared `status` field common to every AgentExecutionEvent variant
      const [working, completed] = eventBus.events as unknown as [
        StatusEvent,
        StatusEvent,
      ]
      expect(eventBus.finishedCalled).toBe(true)
      // No Task-kind event: the existing history must not be clobbered.
      expect(eventBus.events.every((e) => e.kind === 'status-update')).toBe(
        true,
      )
      expect.soft(working.status.state).toBe('working')
      expect.soft(completed.status.state).toBe('completed')
      expect(calls).toEqual([
        {
          strategyId: '11111111-1111-1111-1111-111111111111',
          taskId: 'task-11',
          text: '投資戦略でNVDAを分析して\n長期の方でお願いします',
        },
      ])
    })
  })
})
