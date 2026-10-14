import { describe, expect, it } from 'vitest'

import {
  formatDecimal,
  formatMillionYenInHundredMillions,
  formatRatioAsPercent,
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

describe('formatRatioAsPercent', () => {
  it.each([
    ['null', null, '—'],
    ['a decimal ratio', 0.091, '9.1%'],
    ['a negative decimal ratio', -0.091, '-9.1%'],
  ] as const)('formats %s', (_label, value, expected) => {
    expect(formatRatioAsPercent(value)).toEqual(expected)
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

describe('formatMillionYenInHundredMillions', () => {
  it.each([
    ['null', null, '—'],
    ['millions of yen', 123_456, '1,234.6 億円'],
  ] as const)('formats %s', (_label, value, expected) => {
    expect(formatMillionYenInHundredMillions(value)).toEqual(expected)
  })
})
