import { describe, expect, it } from 'vitest'

import { getBarsRange } from '#lib/chart-range'

describe('getBarsRange', () => {
  const now = new Date(2026, 9, 4, 12, 34, 56)

  it.each([
    { timeframe: '1m', lookbackDays: 4 },
    { timeframe: '5m', lookbackDays: 7 },
    { timeframe: '15m', lookbackDays: 7 },
    { timeframe: '1h', lookbackDays: 30 },
    { timeframe: '4h', lookbackDays: 90 },
  ] as const)(
    'returns an RFC 3339 range for $timeframe with $lookbackDays days of history',
    ({ timeframe, lookbackDays }) => {
      const from = new Date(now)
      from.setDate(from.getDate() - lookbackDays)

      expect(getBarsRange(timeframe, now)).toEqual({
        from: from.toISOString(),
        to: now.toISOString(),
      })
    },
  )

  it('returns calendar dates for daily bars', () => {
    expect(getBarsRange('1d', now)).toEqual({
      from: '2025-10-04',
      to: '2026-10-04',
    })
  })
})
