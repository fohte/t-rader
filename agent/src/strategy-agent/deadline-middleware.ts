import { createAbortingModelCallMiddleware } from '#strategy-agent/aborting-model-call-middleware'
import {
  isPhaseTimeoutSignal,
  type PhaseTimeout,
  phaseTimeoutMessage,
} from '#strategy-agent/phase-timeout'

const DEADLINE_EXCEEDED_FINGERPRINT = 'deadline-middleware.exceeded'

export const createDeadlineMiddleware = (
  signal: AbortSignal,
  phaseTimeout?: PhaseTimeout,
) =>
  createAbortingModelCallMiddleware(
    'deadlineMiddleware',
    (abortedSignal) =>
      isPhaseTimeoutSignal(abortedSignal, phaseTimeout)
        ? 'phase-timeout'
        : 'deadline',
    () => signal,
    DEADLINE_EXCEEDED_FINGERPRINT,
    (abortedSignal) =>
      isPhaseTimeoutSignal(abortedSignal, phaseTimeout) &&
      phaseTimeout !== undefined
        ? phaseTimeoutMessage(phaseTimeout)
        : 'deadlineMiddleware: aborted model call after strategy task deadline exceeded',
  )
