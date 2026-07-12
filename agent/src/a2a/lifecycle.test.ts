import type { Task } from '@a2a-js/sdk'
import { describe, expect, it, vi } from 'vitest'

import {
  runRetentionSweep,
  runWatchdogSweep,
  startTaskLifecycleJobs,
  type TaskLifecycleStore,
} from '@/a2a/lifecycle'

const buildTask = (id: string): Task => ({
  id,
  contextId: `ctx-${id}`,
  kind: 'task',
  status: { state: 'failed', timestamp: '2026-01-01T00:00:00.000Z' },
})

class FakeStore implements TaskLifecycleStore {
  public readonly failStaleWorkingTasksCalls: Date[] = []
  public readonly deleteSettledOlderThanCalls: Date[] = []

  constructor(
    private readonly staleTasks: Task[] = [],
    private readonly deletedCount = 0,
  ) {}

  failStaleWorkingTasks(olderThan: Date): Promise<Task[]> {
    this.failStaleWorkingTasksCalls.push(olderThan)
    return Promise.resolve(this.staleTasks)
  }

  deleteSettledOlderThan(olderThan: Date): Promise<number> {
    this.deleteSettledOlderThanCalls.push(olderThan)
    return Promise.resolve(this.deletedCount)
  }
}

describe('runWatchdogSweep', () => {
  it('computes the heartbeat-stop threshold and notifies onExpire for each failed task', async () => {
    const stale = [buildTask('a'), buildTask('b')]
    const store = new FakeStore(stale)
    const notified: Task[] = []
    const now = () => new Date('2026-01-01T00:10:00.000Z')

    const result = await runWatchdogSweep(
      store,
      5 * 60 * 1000,
      (task) => {
        notified.push(task)
        return Promise.resolve()
      },
      now,
    )

    const actual = {
      result,
      notified,
      thresholdArg: store.failStaleWorkingTasksCalls,
    }
    expect(actual).toEqual({
      result: stale,
      notified: stale,
      thresholdArg: [new Date('2026-01-01T00:05:00.000Z')],
    })
  })
})

describe('runRetentionSweep', () => {
  it('computes the retention-days threshold and returns the deleted count', async () => {
    const store = new FakeStore([], 3)
    const now = () => new Date('2026-01-10T00:00:00.000Z')

    const deletedCount = await runRetentionSweep(store, 7, now)

    const actual = {
      deletedCount,
      thresholdArg: store.deleteSettledOlderThanCalls,
    }
    expect(actual).toEqual({
      deletedCount: 3,
      thresholdArg: [new Date('2026-01-03T00:00:00.000Z')],
    })
  })
})

describe('startTaskLifecycleJobs', () => {
  it('runs an immediate sweep and then on every interval tick, stopping cleanly', async () => {
    vi.useFakeTimers()
    try {
      const store = new FakeStore([buildTask('a')], 1)
      const onExpire = vi.fn(() => Promise.resolve())

      const jobs = startTaskLifecycleJobs(store, {
        workingTimeoutMs: 1000,
        retentionDays: 1,
        onExpire,
        sweepIntervalMs: 10_000,
      })
      // Flush the immediate synchronous tick's async work without advancing
      // the interval itself.
      await vi.advanceTimersByTimeAsync(0)
      expect(store.failStaleWorkingTasksCalls).toHaveLength(1)

      await vi.advanceTimersByTimeAsync(10_000)
      expect(store.failStaleWorkingTasksCalls).toHaveLength(2)

      await jobs.stop()
      await vi.advanceTimersByTimeAsync(30_000)
      expect(store.failStaleWorkingTasksCalls).toHaveLength(2)
    } finally {
      vi.useRealTimers()
    }
  })
})
