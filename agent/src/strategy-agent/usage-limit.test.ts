import { describe, expect, it } from 'vitest'

import { isUsageLimitError } from '@/strategy-agent/usage-limit'

describe('isUsageLimitError', () => {
  it('returns true for rateLimitType "stop" (quota exhausted)', () => {
    const error = Object.assign(new Error('rate limited'), {
      rateLimitType: 'stop',
    })
    expect(isUsageLimitError(error)).toBe(true)
  })

  it('returns true for rateLimitType "capacity" (Retry-After too large)', () => {
    const error = Object.assign(new Error('rate limited'), {
      rateLimitType: 'capacity',
    })
    expect(isUsageLimitError(error)).toBe(true)
  })

  it('returns false for rateLimitType "wait" (retried internally, never reaches the caller)', () => {
    const error = Object.assign(new Error('rate limited'), {
      rateLimitType: 'wait',
    })
    expect(isUsageLimitError(error)).toBe(false)
  })

  it('returns false for a plain Error without rate limit metadata', () => {
    expect(isUsageLimitError(new Error('boom'))).toBe(false)
  })

  it('returns false for non-object values', () => {
    const actual = ['boom', null, undefined].map(isUsageLimitError)
    expect(actual).toEqual([false, false, false])
  })
})
