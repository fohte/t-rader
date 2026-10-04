import { describe, expect, it } from 'vitest'

import { getBarsRange } from '#lib/chart-range'

describe('getBarsRange', () => {
  it('returns RFC 3339 datetimes for intraday bars', () => {
    const now = new Date(2026, 9, 4, 12, 34, 56)

    expect(getBarsRange('1m', now)).toEqual({
      from: new Date(2026, 9, 3, 12, 34, 56).toISOString(),
      to: now.toISOString(),
    })
  })

  it('returns calendar dates for daily bars', () => {
    const now = new Date(2026, 9, 4, 12, 34, 56)

    expect(getBarsRange('1d', now)).toEqual({
      from: '2025-10-04',
      to: '2026-10-04',
    })
  })
})
