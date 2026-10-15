import { describe, expect, it } from 'vitest'

import { extendBarsRange, getBarsRange } from '#lib/chart-range'

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

  it('extends daily history without changing the displayed range end', () => {
    expect(extendBarsRange(getBarsRange('1d', now), '1d', 25)).toEqual({
      from: '2025-08-27',
      to: '2026-10-04',
    })
  })

  it('extends intraday history by enough calendar days for the requested bars', () => {
    expect(extendBarsRange(getBarsRange('5m', now), '5m', 25)).toEqual({
      from: '2026-09-26T03:34:56.000Z',
      to: '2026-10-04T03:34:56.000Z',
    })
  })

  it('leaves the range unchanged when no indicator bars are needed', () => {
    const range = getBarsRange('5m', now)

    expect(extendBarsRange(range, '5m', 0)).toEqual(range)
  })
})
