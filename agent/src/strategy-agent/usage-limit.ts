// The chat model is built with maxRetries: 0 (see strategy-agent.ts), so
// LangChain core's AsyncCaller never retries a 429 itself — every 429 reaches
// us on its first failed attempt, stamped with
// `rateLimitType: 'wait' | 'stop' | 'capacity'` (verified against
// langchain-core's async_caller.ts: a short Retry-After uses 'wait',
// InsufficientQuotaError and RateLimitQuotaExhaustedError use 'stop',
// RateLimitCapacityError 'capacity').
export const isUsageLimitError = (error: unknown): boolean => {
  if (typeof error !== 'object' || error === null) return false
  // eslint-disable-next-line @typescript-eslint/no-unsafe-type-assertion -- error is an untyped bag; narrowed immediately below via the equality checks
  const rateLimitType = (error as Record<string, unknown>)['rateLimitType']
  return (
    rateLimitType === 'wait' ||
    rateLimitType === 'stop' ||
    rateLimitType === 'capacity'
  )
}
