import { createAbortingModelCallMiddleware } from '#strategy-agent/aborting-model-call-middleware'

const DEADLINE_EXCEEDED_FINGERPRINT = 'deadline-middleware.exceeded'

export const createDeadlineMiddleware = (signal: AbortSignal) =>
  createAbortingModelCallMiddleware(
    'deadlineMiddleware',
    () => signal,
    DEADLINE_EXCEEDED_FINGERPRINT,
    () =>
      'deadlineMiddleware: aborted model call after strategy task deadline exceeded',
  )
