import type { Task, TaskState } from '@a2a-js/sdk'
import type { TaskStore } from '@a2a-js/sdk/server'
import { and, eq, inArray, lt, notInArray } from 'drizzle-orm'
import { drizzle } from 'drizzle-orm/postgres-js'

import type { Sql } from '#db'
import { a2aPushConfigs, a2aTasks } from '#db/schema'

// Task rows in these states are done for good; `save()` never overwrites
// them so a watchdog-driven failure can't be clobbered by a still-running
// executor that hasn't noticed it lost the race, and vice versa.
const TERMINAL_STATES: readonly TaskState[] = [
  'completed',
  'failed',
  'canceled',
  'rejected',
]

// input-required is not terminal (a task resumes from it via an additional
// message/send), but a task left there indefinitely is still eligible for
// retention cleanup alongside the terminal states.
const RETENTION_ELIGIBLE_STATES: readonly TaskState[] = [
  ...TERMINAL_STATES,
  'input-required',
]

const statusTimestampOf = (task: Task): Date =>
  task.status.timestamp !== undefined
    ? new Date(task.status.timestamp)
    : new Date()

export class PostgresTaskStore implements TaskStore {
  private readonly db: ReturnType<typeof drizzle>

  constructor(sql: Sql) {
    this.db = drizzle(sql)
  }

  async save(task: Task): Promise<void> {
    const row = {
      taskId: task.id,
      contextId: task.contextId,
      state: task.status.state,
      statusTimestamp: statusTimestampOf(task),
      task,
    }
    await this.db
      .insert(a2aTasks)
      .values(row)
      .onConflictDoUpdate({
        target: a2aTasks.taskId,
        set: {
          contextId: row.contextId,
          state: row.state,
          statusTimestamp: row.statusTimestamp,
          task: row.task,
        },
        setWhere: notInArray(a2aTasks.state, [...TERMINAL_STATES]),
      })
  }

  async load(taskId: string): Promise<Task | undefined> {
    const [row] = await this.db
      .select({ task: a2aTasks.task })
      .from(a2aTasks)
      .where(eq(a2aTasks.taskId, taskId))
      .limit(1)
    return row?.task
  }

  // Fails tasks stuck in `working` whose heartbeat (status_timestamp) has
  // not advanced past `olderThan`. The candidate list comes from a plain
  // read, but each row is only written via an UPDATE re-guarded on the same
  // `state = 'working' AND status_timestamp < olderThan` condition — so a
  // heartbeat published between the read and the write makes that row's
  // guard fail and the stale write is simply skipped, keeping this race-free
  // against a still-live executor.
  async failStaleWorkingTasks(olderThan: Date): Promise<Task[]> {
    const candidates = await this.db
      .select({ taskId: a2aTasks.taskId, task: a2aTasks.task })
      .from(a2aTasks)
      .where(
        and(
          eq(a2aTasks.state, 'working'),
          lt(a2aTasks.statusTimestamp, olderThan),
        ),
      )

    const failedAt = new Date()
    const results: Task[] = []
    for (const candidate of candidates) {
      const failedTask: Task = {
        ...candidate.task,
        status: {
          ...candidate.task.status,
          state: 'failed',
          timestamp: failedAt.toISOString(),
        },
      }
      const updated = await this.db
        .update(a2aTasks)
        .set({ state: 'failed', statusTimestamp: failedAt, task: failedTask })
        .where(
          and(
            eq(a2aTasks.taskId, candidate.taskId),
            eq(a2aTasks.state, 'working'),
            lt(a2aTasks.statusTimestamp, olderThan),
          ),
        )
        .returning({ taskId: a2aTasks.taskId })
      if (updated.length > 0) results.push(failedTask)
    }
    return results
  }

  // Deletes rows that are done (terminal, or input-required left unresumed)
  // and past the retention window, along with their push notification
  // configs (no FK/cascade here — see the comment on a2aPushConfigs).
  async deleteSettledOlderThan(olderThan: Date): Promise<number> {
    const deletedTasks = await this.db
      .delete(a2aTasks)
      .where(
        and(
          inArray(a2aTasks.state, [...RETENTION_ELIGIBLE_STATES]),
          lt(a2aTasks.statusTimestamp, olderThan),
        ),
      )
      .returning({ taskId: a2aTasks.taskId })

    if (deletedTasks.length > 0) {
      await this.db.delete(a2aPushConfigs).where(
        inArray(
          a2aPushConfigs.taskId,
          deletedTasks.map((t) => t.taskId),
        ),
      )
    }
    return deletedTasks.length
  }
}
