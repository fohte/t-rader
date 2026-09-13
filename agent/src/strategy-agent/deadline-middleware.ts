import { createAbortingModelCallMiddleware } from '#strategy-agent/aborting-model-call-middleware'

const DEADLINE_EXCEEDED_FINGERPRINT = 'deadline-middleware.exceeded'

// モデル呼び出しに signal を合成し、abort 時に警告ログとメトリクスを記録する。
export const createDeadlineMiddleware = (signal: AbortSignal) =>
  createAbortingModelCallMiddleware(
    'deadlineMiddleware',
    signal,
    DEADLINE_EXCEEDED_FINGERPRINT,
    () =>
      'deadlineMiddleware: aborted model call after strategy task deadline exceeded',
  )
