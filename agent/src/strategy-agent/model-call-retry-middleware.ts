import { type AnyAgentMiddleware, modelRetryMiddleware } from 'langchain'

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

const isRetryableModelCallError = (error: Error): boolean => {
  const errorChain = getErrorChain(error)

  if (
    errorChain.some(
      (cause) =>
        cause instanceof AbortedModelCallError && cause.reason === 'deadline',
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
