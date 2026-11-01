import { describe, expect, it } from 'vitest'

import type { components } from '#lib/api/schema.gen'
import {
  findDailyBarForDate,
  getDailyBarsRangeAroundDate,
  validateStockPageSearch,
} from '#lib/stock-page-date'

type Bar = components['schemas']['Bar']

const bar: Bar = {
  close: 3060,
  high: 3100,
  instrument_id: 'demo-code',
  low: 2980,
  open: 3000,
  timeframe: '1d',
  timestamp: '2030-04-02T15:00:00Z',
  volume: 10000,
}

describe('validateStockPageSearch', () => {
  it.each([
    ['valid date', { date: '2030-04-03' }, { date: '2030-04-03' }],
    [
      'financial period with chart date',
      {
        date: '2030-04-03',
        stockTab: 'financials',
        periodEnd: '2030-03-31',
        periodType: 'FY',
      },
      {
        date: '2030-04-03',
        stockTab: 'financials',
        periodEnd: '2030-03-31',
        periodType: 'FY',
      },
    ],
    [
      'quarter tab without target period',
      { stockTab: 'financials' },
      { stockTab: 'financials' },
    ],
    ['invalid format', { date: '2030-4-3' }, {}],
    ['impossible date', { date: '2030-02-30' }, {}],
    ['non-string date', { date: ['2030-04-03'] }, {}],
    ['period without its type', { periodEnd: '2030-03-31' }, {}],
    [
      'invalid target period',
      { periodEnd: '2030-02-30', periodType: 'FY' },
      {},
    ],
    [
      'unsupported period type',
      { periodEnd: '2030-03-31', periodType: 'H1' },
      {},
    ],
    ['unsupported tab', { stockTab: 'history' }, {}],
  ])('normalizes the stock page URL for %s', (_name, search, expected) => {
    expect(validateStockPageSearch(search)).toEqual(expected)
  })
})

describe('getDailyBarsRangeAroundDate', () => {
  it('centers an inclusive 365-day range on the requested date', () => {
    expect(getDailyBarsRangeAroundDate('2030-04-03')).toEqual({
      from: '2029-10-03',
      to: '2030-10-02',
    })
  })
})

describe('findDailyBarForDate', () => {
  it.each([
    ['matching Tokyo date', '2030-04-03', bar],
    ['date without a daily bar', '2030-04-04', null],
  ])('returns the daily bar for a %s', (_name, date, expected) => {
    expect(findDailyBarForDate([bar], date)).toEqual(expected)
  })
})
