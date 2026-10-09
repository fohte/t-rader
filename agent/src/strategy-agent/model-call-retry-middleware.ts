import { type AnyAgentMiddleware, createMiddleware } from 'langchain'

import { isRecord } from '#errors'
import { logger } from '#logger'
import { AbortedModelCallError } from '#strategy-agent/aborting-model-call-middleware'
import { isUsageLimitError } from '#strategy-agent/usage-limit'

const CONNECTION_ERROR_CODES = new Set([
  'ECONNRESET',
  'ECONNREFUSED',
  'EPIPE',
  'ETIMEDOUT',
  'ENETUNREACH',
  'UND_ERR_SOCKET',
  'UND_ERR_CONNECT_TIMEOUT',
  'UND_ERR_HEADERS_TIMEOUT',
  'UND_ERR_BODY_TIMEOUT',
])

const MAX_RETRIES = 2
const MAX_CALL_DURATION_RETRIES = 1
const RETRY_INITIAL_DELAY_MS = 1_000
const RETRY_BACKOFF_FACTOR = 2
const RETRY_MAX_DELAY_MS = 60_000

const getErrorChain = (error: Error): Error[] => {
  const chain: Error[] = []
  const visited = new Set<Error>()
  let current: unknown = error

  while (current instanceof Error && !visited.has(current)) {
    chain.push(current)
    visited.add(current)
    current = current.cause
  }

  return chain
}

const getStatus = (error: Error): number | undefined => {
  const status = isRecord(error) ? error['status'] : undefined
  return typeof status === 'number' ? status : undefined
}

const getErrorCode = (error: unknown): string | undefined => {
  const code = isRecord(error) ? error['code'] : undefined
  return typeof code === 'string' ? code : undefined
}

const isTransportTypeError = (error: Error): boolean =>
  error instanceof TypeError &&
  (error.message === 'terminated' ||
    CONNECTION_ERROR_CODES.has(getErrorCode(error) ?? '') ||
    CONNECTION_ERROR_CODES.has(getErrorCode(error.cause) ?? ''))

const isStatuslessSseApiError = (error: Error): boolean =>
  isRecord(error) &&
  error['status'] === undefined &&
  error['headers'] instanceof Headers &&
  isRecord(error['error'])

const getOriginalError = (error: Error): Error => {
  let originalError = error
  for (const cause of getErrorChain(error)) originalError = cause
  return originalError
}

const isCallDurationAbort = (error: Error): boolean =>
  getErrorChain(error).some(
    (cause) =>
      cause instanceof AbortedModelCallError &&
      cause.reason === 'call-duration',
  )

const getRetryDelay = (error: Error, retryNumber: number): number => {
  const delay = Math.min(
    RETRY_INITIAL_DELAY_MS * RETRY_BACKOFF_FACTOR ** retryNumber,
    RETRY_MAX_DELAY_MS,
  )
  const jitteredDelay = Math.max(
    0,
    delay + (Math.random() * 2 - 1) * delay * 0.25,
  )
  const retryAfterMs = isRecord(error) ? error['retryAfterMs'] : undefined

  return Math.max(
    jitteredDelay,
    typeof retryAfterMs === 'number' && retryAfterMs >= 0 ? retryAfterMs : 0,
  )
}

const waitForRetry = (delayMs: number): Promise<void> =>
  new Promise((resolve) => setTimeout(resolve, delayMs))

const isRetryableModelCallError = (error: Error): boolean => {
  const errorChain = getErrorChain(error)

  if (
    errorChain.some(
      (cause) =>
        cause instanceof AbortedModelCallError && cause.reason === 'deadline',
    ) ||
    isUsageLimitError(error)
  ) {
    return false
  }

  let status: number | undefined
  for (let index = errorChain.length - 1; index >= 0; index -= 1) {
    const cause = errorChain[index]
    if (cause === undefined) continue
    status = getStatus(cause)
    if (status !== undefined) break
  }
  if (status !== undefined) return status >= 500

  if (
    errorChain.some(
      (cause) =>
        cause instanceof AbortedModelCallError &&
        cause.reason === 'call-duration',
    )
  ) {
    return true
  }

  return errorChain.some(
    (cause) =>
      cause.name === 'TimeoutError' ||
      cause.constructor.name === 'APIConnectionError' ||
      cause.constructor.name === 'APIConnectionTimeoutError' ||
      isTransportTypeError(cause) ||
      isStatuslessSseApiError(cause),
  )
}

export const createModelCallRetryMiddleware = (): AnyAgentMiddleware => {
  return createMiddleware({
    name: 'modelCallRetryMiddleware',
    wrapModelCall: (request, handler) => {
      const delegated = {
        ...request,
        modelSettings: { maxRetries: 0, ...request.modelSettings },
      }

      const callModel = (
        retriesMade: number,
        callDurationRetriesMade: number,
      ): ReturnType<typeof handler> =>
        Promise.resolve()
          .then(() => handler(delegated))
          .catch(async (caught: unknown) => {
            const error =
              caught instanceof Error ? caught : new Error(String(caught))
            const isCallDuration = isCallDurationAbort(error)

            if (
              !isRetryableModelCallError(error) ||
              retriesMade >= MAX_RETRIES ||
              (isCallDuration &&
                callDurationRetriesMade >= MAX_CALL_DURATION_RETRIES)
            ) {
              return Promise.reject(error)
            }

            logger.warn(
              { err: getOriginalError(error) },
              'model call failed with a retryable error',
            )
            await waitForRetry(getRetryDelay(error, retriesMade))

            return callModel(
              retriesMade + 1,
              callDurationRetriesMade + Number(isCallDuration),
            )
          })

      return callModel(0, 0)
    },
  })
}
