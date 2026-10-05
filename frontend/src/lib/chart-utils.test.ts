import { describe, expect, it } from 'vitest'

import { getChartCurrency } from '#lib/chart-utils'

describe('getChartCurrency', () => {
  it('maps supported markets to their price currencies', () => {
    const currencies = Array.of(
      getChartCurrency('TSE'),
      getChartCurrency('US'),
      getChartCurrency('OTHER'),
      getChartCurrency(null),
    ).map((currency) => currency ?? null)

    expect(JSON.stringify(currencies)).toBe('["JPY","USD",null,null]')
  })
})
