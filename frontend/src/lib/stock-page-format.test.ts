import { describe, expect, it } from 'vitest'

import {
  formatDecimal,
  formatYenInHundredMillions,
} from '#lib/stock-page-format'

describe('formatDecimal', () => {
  it.each([
    ['null', null, '—'],
    ['a decimal number', 16.2, '16.2'],
  ] as const)('formats %s', (_label, value, expected) => {
    expect(formatDecimal(value)).toEqual(expected)
  })
})

describe('formatYenInHundredMillions', () => {
  it.each([
    ['null', null, '—'],
    ['below one hundred million yen', 1200, '1,200 円'],
    ['hundred millions of yen', 820_000_000_000, '8,200 億円'],
  ] as const)('formats %s', (_label, value, expected) => {
    expect(formatYenInHundredMillions(value)).toEqual(expected)
  })
})
