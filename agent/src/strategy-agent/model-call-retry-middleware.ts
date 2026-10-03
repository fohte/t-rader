import { type AnyAgentMiddleware, modelRetryMiddleware } from 'langchain'

import { logger } from '#logger'
import { isUsageLimitError } from '#strategy-agent/usage-limit'

const DEADLINE_ERROR_PREFIX =
  'deadlineMiddleware: aborted model call after strategy task deadline exceeded'
const CALL_DURATION_ERROR_PREFIX =
  'callDurationMiddleware: aborted model call after exceeding '

const isRecord = (value: unknown): value is Record<string, unknown> =>
  typeof value === 'object' && value !== null

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

const isRetryableModelCallError = (error: Error): boolean => {
  const errorChain = getErrorChain(error)

  if (
    errorChain.some((cause) =>
      cause.message.startsWith(DEADLINE_ERROR_PREFIX),
    ) ||
    errorChain.some(isUsageLimitError)
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
    errorChain.some((cause) =>
      cause.message.startsWith(CALL_DURATION_ERROR_PREFIX),
    )
  ) {
    return true
  }

  return errorChain.some(
    (cause) =>
      cause.name === 'TimeoutError' ||
      cause.constructor.name === 'APIConnectionError' ||
      cause.constructor.name === 'APIConnectionTimeoutError' ||
      cause instanceof TypeError ||
      isStatuslessSseApiError(cause),
  )
}

export const createModelCallRetryMiddleware = (): AnyAgentMiddleware => {
  // modelRetryMiddleware の公開型が Zod v3 に固定され、createAgent の interop 型と一致しない。
  // eslint-disable-next-line @typescript-eslint/no-unsafe-type-assertion -- 公開型の不整合を吸収する。middleware 実装は createAgent と同じ LangChain パッケージから取得している。
  return modelRetryMiddleware({
    maxRetries: 2,
    onFailure: 'error',
    retryOn: (error) => {
      if (!isRetryableModelCallError(error)) return false
      logger.warn(
        { err: getOriginalError(error) },
        'model call failed with a retryable error',
      )
      return true
    },
  }) as unknown as AnyAgentMiddleware
}
