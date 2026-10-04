import type { Task } from '@a2a-js/sdk'

const DAY_MS = 24 * 60 * 60 * 1000

export interface TaskLifecycleStore {
  failStaleWorkingTasks(olderThan: Date): Promise<Task[]>
  deleteSettledOlderThan(olderThan: Date): Promise<number>
}

export const runWatchdogSweep = async (
  store: TaskLifecycleStore,
  workingTimeoutMs: number,
  onExpire: (task: Task) => Promise<void>,
  now: () => Date = () => new Date(),
): Promise<Task[]> => {
  const threshold = new Date(now().getTime() - workingTimeoutMs)
  const expired = await store.failStaleWorkingTasks(threshold)
  await Promise.all(expired.map((task) => onExpire(task)))
  return expired
}

export const runRetentionSweep = async (
  store: TaskLifecycleStore,
  retentionDays: number,
  now: () => Date = () => new Date(),
): Promise<number> => {
  const threshold = new Date(now().getTime() - retentionDays * DAY_MS)
  return store.deleteSettledOlderThan(threshold)
}
