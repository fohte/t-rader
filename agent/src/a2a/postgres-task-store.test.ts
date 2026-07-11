import type { Task, TaskState } from '@a2a-js/sdk'
import { describe, expect, it } from 'vitest'

import { PostgresPushNotificationStore } from '@/a2a/postgres-push-notification-store'
import { PostgresTaskStore } from '@/a2a/postgres-task-store'
import { describeIfDb, setupDrizzleTx } from '@/test/db'

const buildTask = (overrides: {
  id: string
  contextId?: string
  state: TaskState
  timestamp: string
}): Task => ({
  id: overrides.id,
  contextId: overrides.contextId ?? `ctx-${overrides.id}`,
  kind: 'task',
  status: { state: overrides.state, timestamp: overrides.timestamp },
})

describeIfDb('PostgresTaskStore', () => {
  const getTx = setupDrizzleTx()

  describe('save / load', () => {
    it('round-trips a task', async () => {
      const store = new PostgresTaskStore(getTx())
      const task = buildTask({
        id: 'task-1',
        state: 'submitted',
        timestamp: '2026-01-01T00:00:00.000Z',
      })

      await store.save(task)

      expect(await store.load('task-1')).toEqual(task)
    })

    it('returns undefined for an unknown task id', async () => {
      const store = new PostgresTaskStore(getTx())
      expect(await store.load('does-not-exist')).toBeUndefined()
    })

    it('updates a non-terminal row in place', async () => {
      const store = new PostgresTaskStore(getTx())
      await store.save(
        buildTask({
          id: 'task-2',
          state: 'submitted',
          timestamp: '2026-01-01T00:00:00.000Z',
        }),
      )
      const working = buildTask({
        id: 'task-2',
        state: 'working',
        timestamp: '2026-01-01T00:01:00.000Z',
      })

      await store.save(working)

      expect(await store.load('task-2')).toEqual(working)
    })

    it('does not overwrite a terminal row with a later non-terminal save', async () => {
      const store = new PostgresTaskStore(getTx())
      const completed = buildTask({
        id: 'task-3',
        state: 'completed',
        timestamp: '2026-01-01T00:00:00.000Z',
      })
      await store.save(completed)

      await store.save(
        buildTask({
          id: 'task-3',
          state: 'working',
          timestamp: '2026-01-01T00:01:00.000Z',
        }),
      )

      expect(await store.load('task-3')).toEqual(completed)
    })
  })

  describe('failStaleWorkingTasks', () => {
    it('fails only working rows whose heartbeat is older than the threshold', async () => {
      const store = new PostgresTaskStore(getTx())
      await store.save(
        buildTask({
          id: 'stale',
          state: 'working',
          timestamp: '2026-01-01T00:00:00.000Z',
        }),
      )
      await store.save(
        buildTask({
          id: 'fresh',
          state: 'working',
          timestamp: '2026-01-01T00:09:00.000Z',
        }),
      )
      await store.save(
        buildTask({
          id: 'already-done',
          state: 'completed',
          timestamp: '2026-01-01T00:00:00.000Z',
        }),
      )

      const expired = await store.failStaleWorkingTasks(
        new Date('2026-01-01T00:05:00.000Z'),
      )

      expect({
        expiredIds: expired.map((t) => t.id).sort(),
        expiredStates: expired.map((t) => t.status.state),
        stale: await store.load('stale'),
        fresh: await store.load('fresh'),
        alreadyDone: await store.load('already-done'),
      }).toEqual({
        expiredIds: ['stale'],
        expiredStates: ['failed'],
        stale: {
          id: 'stale',
          contextId: 'ctx-stale',
          kind: 'task',
          status: {
            state: 'failed',
            timestamp: expired[0]?.status.timestamp ?? '',
          },
        },
        fresh: buildTask({
          id: 'fresh',
          state: 'working',
          timestamp: '2026-01-01T00:09:00.000Z',
        }),
        alreadyDone: buildTask({
          id: 'already-done',
          state: 'completed',
          timestamp: '2026-01-01T00:00:00.000Z',
        }),
      })
    })
  })

  describe('deleteSettledOlderThan', () => {
    it('deletes terminal and input-required rows past the retention window, leaving active rows', async () => {
      const store = new PostgresTaskStore(getTx())
      await store.save(
        buildTask({
          id: 'old-completed',
          state: 'completed',
          timestamp: '2026-01-01T00:00:00.000Z',
        }),
      )
      await store.save(
        buildTask({
          id: 'old-input-required',
          state: 'input-required',
          timestamp: '2026-01-01T00:00:00.000Z',
        }),
      )
      await store.save(
        buildTask({
          id: 'recent-completed',
          state: 'completed',
          timestamp: '2026-01-10T00:00:00.000Z',
        }),
      )
      await store.save(
        buildTask({
          id: 'still-working',
          state: 'working',
          timestamp: '2026-01-01T00:00:00.000Z',
        }),
      )

      // a2a_push_configs has no FK to a2a_tasks (DefaultRequestHandler saves
      // the push config before the task row exists), so retention must clean
      // these up itself rather than relying on ON DELETE CASCADE.
      const pushStore = new PostgresPushNotificationStore(getTx())
      await pushStore.save('old-completed', { url: 'https://example.com/old' })
      await pushStore.save('recent-completed', {
        url: 'https://example.com/recent',
      })

      const deletedCount = await store.deleteSettledOlderThan(
        new Date('2026-01-05T00:00:00.000Z'),
      )

      expect({
        deletedCount,
        oldCompleted: await store.load('old-completed'),
        oldInputRequired: await store.load('old-input-required'),
        recentCompleted: await store.load('recent-completed'),
        stillWorking: await store.load('still-working'),
        oldCompletedPushConfigs: await pushStore.load('old-completed'),
        recentCompletedPushConfigs: await pushStore.load('recent-completed'),
      }).toEqual({
        deletedCount: 2,
        oldCompleted: undefined,
        oldInputRequired: undefined,
        recentCompleted: buildTask({
          id: 'recent-completed',
          state: 'completed',
          timestamp: '2026-01-10T00:00:00.000Z',
        }),
        stillWorking: buildTask({
          id: 'still-working',
          state: 'working',
          timestamp: '2026-01-01T00:00:00.000Z',
        }),
        oldCompletedPushConfigs: [],
        recentCompletedPushConfigs: [
          { id: 'recent-completed', url: 'https://example.com/recent' },
        ],
      })
    })
  })
})
