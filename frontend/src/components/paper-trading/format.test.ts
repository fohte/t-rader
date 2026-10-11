import { describe, expect, it } from 'vitest'

import { formatPercent } from '#components/paper-trading/format'

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
