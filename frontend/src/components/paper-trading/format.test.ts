import { describe, expect, it } from 'vitest'

import {
  formatOptionalPercent,
  formatPercent,
  formatWinRate,
  optionalPnlColorClass,
} from '#components/paper-trading/format'

describe('formatPercent', () => {
  it.each([
    ['positive signed return', 0.125, true, '+12.5%'],
    ['negative signed return', -0.02, true, '−2.0%'],
    ['zero signed return', 0, true, '0.0%'],
    ['win rate', 0.625, false, '62.5%'],
  ] as const)('formats a %s as %s', (_caseName, value, signed, expected) => {
    expect(formatPercent(value, signed)).toBe(expected)
  })
})

describe('formatOptionalPercent', () => {
  it.each([
    ['missing percent', null, true, '—'],
    ['unsigned win rate', 0.625, false, '62.5%'],
  ] as const)('formats a %s as %s', (_caseName, value, signed, expected) => {
    expect(formatOptionalPercent(value, signed)).toBe(expected)
  })
})

describe('formatWinRate', () => {
  it.each([
    ['account without closed trades', 0, 1, '—'],
    ['missing win rate', 2, null, '—'],
    ['account with closed trades', 8, 0.625, '62.5%'],
  ] as const)(
    'formats a %s as %s',
    (_caseName, closedTradeCount, winRate, expected) => {
      expect(formatWinRate(closedTradeCount, winRate)).toBe(expected)
    },
  )
})

describe('optionalPnlColorClass', () => {
  it.each([
    ['missing value', null, 'text-muted-foreground-strong'],
    ['positive value', 0.1, 'text-up'],
    ['negative value', -0.1, 'text-down'],
  ] as const)('formats a %s as %s', (_caseName, value, expected) => {
    expect(optionalPnlColorClass(value)).toBe(expected)
  })
})
