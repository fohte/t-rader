import type { Task } from '@a2a-js/sdk'
import { captureWithFingerprint } from '@fohte/service-kit/observability'

// Narrow interface (rather than the concrete PostgresTaskStore) so the sweep
// logic here can be tested against a plain fake store.
export interface TaskLifecycleStore {
  failStaleWorkingTasks(olderThan: Date): Promise<Task[]>
  deleteSettledOlderThan(olderThan: Date): Promise<number>
}

export interface TaskLifecycleJobsOptions {
  // watchdog: a `working` task whose heartbeat (status_timestamp) has been
  // silent for longer than this is failed. This is heartbeat-stop time, not
  // a max execution time — a slow-but-alive executor keeps republishing
  // working status-updates to push the heartbeat forward.
  workingTimeoutMs: number
  // retention: terminal (or unresumed input-required) rows older than this
  // many days are deleted.
  retentionDays: number
  // Invoked once per task the watchdog fails, so the caller can push-notify
  // subscribers the same way a normal executor-driven completion would.
  onExpire: (task: Task) => Promise<void>
  sweepIntervalMs?: number
  now?: () => Date
}

export interface TaskLifecycleJobs {
  stop(): Promise<void>
}

const DAY_MS = 24 * 60 * 60 * 1000
const DEFAULT_SWEEP_INTERVAL_MS = 60_000

const WATCHDOG_SWEEP_FAILED_FINGERPRINT = 'a2a.lifecycle.watchdog-sweep-failed'
const RETENTION_SWEEP_FAILED_FINGERPRINT =
  'a2a.lifecycle.retention-sweep-failed'

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

export const startTaskLifecycleJobs = (
  store: TaskLifecycleStore,
  options: TaskLifecycleJobsOptions,
): TaskLifecycleJobs => {
  const sweepIntervalMs = options.sweepIntervalMs ?? DEFAULT_SWEEP_INTERVAL_MS
  const now = options.now ?? (() => new Date())
  let timer: NodeJS.Timeout | undefined
  let stopped = false

  // Recursive setTimeout (not setInterval) so the next sweep is only
  // scheduled once the previous one has fully settled, preventing
  // overlapping sweeps if a sweep ever takes longer than sweepIntervalMs.
  const tick = async (): Promise<void> => {
    await Promise.all([
      runWatchdogSweep(
        store,
        options.workingTimeoutMs,
        options.onExpire,
        now,
      ).catch((err: unknown) => {
        console.error('a2a watchdog sweep failed:', err)
        captureWithFingerprint(err, WATCHDOG_SWEEP_FAILED_FINGERPRINT)
      }),
      runRetentionSweep(store, options.retentionDays, now).catch(
        (err: unknown) => {
          console.error('a2a retention sweep failed:', err)
          captureWithFingerprint(err, RETENTION_SWEEP_FAILED_FINGERPRINT)
        },
      ),
    ])
    if (!stopped) {
      timer = setTimeout(() => {
        void tick()
      }, sweepIntervalMs)
    }
  }

  void tick()

  return {
    stop: () => {
      stopped = true
      if (timer !== undefined) {
        clearTimeout(timer)
      }
      return Promise.resolve()
    },
  }
}
