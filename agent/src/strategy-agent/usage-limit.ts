import { isRecord } from '#errors'

// The chat model is built with maxRetries: 0 (see strategy-agent.ts), so
// LangChain core's AsyncCaller never retries a 429 itself — every 429 reaches
// us on its first failed attempt, stamped with
// `rateLimitType: 'wait' | 'stop' | 'capacity'` (verified against
// langchain-core's async_caller.ts: a short Retry-After uses 'wait',
// InsufficientQuotaError and RateLimitQuotaExhaustedError use 'stop',
// RateLimitCapacityError 'capacity').
export const isUsageLimitError = (error: unknown): boolean => {
  const visited = new Set<object>()
  let current: unknown = error

  while (isRecord(current) && !visited.has(current)) {
    visited.add(current)
    const rateLimitType = current['rateLimitType']
    if (
      rateLimitType === 'wait' ||
      rateLimitType === 'stop' ||
      rateLimitType === 'capacity'
    ) {
      return true
    }
    current = current['cause']
  }

  return false
}
