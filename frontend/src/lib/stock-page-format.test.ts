import { describe, expect, it } from 'vitest'

import { formatYenInHundredMillions } from '#lib/stock-page-format'

describe('formatYenInHundredMillions', () => {
  it.each([
    ['null', null, '—'],
    ['below one hundred million yen', 1200, '1,200 円'],
    ['hundred millions of yen', 820_000_000_000, '8,200 億円'],
  ] as const)('formats %s', (_label, value, expected) => {
    expect(formatYenInHundredMillions(value)).toEqual(expected)
  })
})
