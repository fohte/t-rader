// LangChain core's AsyncCaller retries 429s that carry a short Retry-After
// itself (research/langgraph-runtime.md), so an error only reaches us once
// that budget is exhausted or the provider reports quota exhaustion. Either
// way its failed-attempt handler stamps the thrown error with
// `rateLimitType: 'stop' | 'capacity'` before giving up (verified against
// langchain-core's async_caller.ts: InsufficientQuotaError uses 'stop',
// RateLimitQuotaExhaustedError 'stop', RateLimitCapacityError 'capacity').
// A 'wait' classification never reaches the caller because AsyncCaller
// retries it internally instead of throwing.
export const isUsageLimitError = (error: unknown): boolean => {
  if (typeof error !== 'object' || error === null) return false
  // eslint-disable-next-line @typescript-eslint/no-unsafe-type-assertion -- error is an untyped bag; narrowed immediately below via the equality checks
  const rateLimitType = (error as Record<string, unknown>)['rateLimitType']
  return rateLimitType === 'stop' || rateLimitType === 'capacity'
}
