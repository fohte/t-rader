import { describe, expect, it } from 'vitest'

import { TRADE_DETAIL_CHART_BARS } from '#components/trades/trade-detail-chart.fixtures'
import {
  createTradeDetailChartState,
  createTradeReflectionDraft,
  formatTradeContextLabel,
  getTradeDetailChartRange,
} from '#components/trades/trade-detail-model'
import type { components } from '#lib/api/schema.gen'

type TradeDetail = components['schemas']['TradeDetailResponse']

const sampleTrade: TradeDetail = {
  id: 'fake-trade-sell',
  strategy_id: 'fake-strategy',
  symbol: 'FIC-01',
  side: 'sell',
  qty: 10,
  price: 1000,
  fee: 10,
  date: '2099-02-10',
  source: 'manual',
  note: null,
  created_at: '2099-02-10T00:00:00Z',
  updated_at: '2099-02-10T00:00:00Z',
  realized_pnl: 2200,
  matched_buys: [{ date: '2099-02-01', qty: 10, price: 780 }],
}

describe('formatTradeContextLabel', () => {
  it.each([
    ['sell', '架空銘柄', '2099-02-10 売り 架空銘柄'],
    ['buy', null, '2099-02-10 買い FIC-01'],
  ] as const)(
    'formats a %s trade with the available stock label',
    (side, stockName, expected) => {
      expect(formatTradeContextLabel({ ...sampleTrade, side }, stockName)).toBe(
        expected,
      )
    },
  )
})

const chartTrade: TradeDetail = {
  id: 'fake-trade-buy',
  strategy_id: 'fake-strategy',
  symbol: 'FIC-01',
  side: 'buy',
  qty: 10,
  price: 1000,
  fee: 10,
  date: '2099-02-10',
  source: 'manual',
  note: null,
  created_at: '2099-02-10T00:00:00Z',
  updated_at: '2099-02-10T00:00:00Z',
  remaining_qty: 10,
}

describe('getTradeDetailChartRange', () => {
  it.each([
    [
      'a date within the month',
      '2099-02-10',
      { from: '2099-01-27', to: '2099-02-24' },
    ],
    [
      'a range crossing the year boundary',
      '2099-01-05',
      { from: '2098-12-22', to: '2099-01-19' },
    ],
    ['an absent date', undefined, null],
  ])('returns the 14-day range for %s', (_case, date, expected) => {
    expect(getTradeDetailChartRange(date)).toEqual(expected)
  })
})

describe('createTradeDetailChartState', () => {
  it.each([
    [
      'loading',
      { bars: undefined, barsPending: true, barsError: false },
      { status: 'loading' },
    ],
    [
      'error',
      { bars: undefined, barsPending: false, barsError: true },
      { status: 'error' },
    ],
    [
      'empty',
      { bars: [], barsPending: false, barsError: false },
      { status: 'empty' },
    ],
  ])(
    'uses daily bars to determine the %s state',
    (_case, chartInput, expected) => {
      expect(
        createTradeDetailChartState({
          ...chartInput,
          trades: undefined,
          selectedTrade: chartTrade,
        }),
      ).toEqual(expected)
    },
  )

  it('keeps the selected trade visible while the symbol trade list is unavailable', () => {
    expect(
      createTradeDetailChartState({
        bars: TRADE_DETAIL_CHART_BARS,
        barsPending: false,
        barsError: false,
        trades: undefined,
        selectedTrade: chartTrade,
      }),
    ).toEqual({
      status: 'ready',
      bars: TRADE_DETAIL_CHART_BARS,
      trades: [{ id: 'fake-trade-buy', side: 'buy', date: '2099-02-10' }],
    })
  })
})

describe('createTradeReflectionDraft', () => {
  it('includes sale facts, realized profit, and a stock reference', () => {
    const trade: TradeDetail = {
      id: 'fake-trade-sell',
      strategy_id: 'fake-strategy',
      symbol: 'FIC-01',
      side: 'sell',
      qty: 10,
      price: 1000,
      fee: 10,
      date: '2099-02-10',
      source: 'manual',
      note: null,
      created_at: '2099-02-10T00:00:00Z',
      updated_at: '2099-02-10T00:00:00Z',
      realized_pnl: 2200,
      matched_buys: [{ date: '2099-02-01', qty: 10, price: 780 }],
    }

    expect(createTradeReflectionDraft(trade, '架空銘柄')).toEqual({
      title: '架空銘柄を売った振り返り',
      body: [
        '2099-02-10 売 10 株 @¥1,000',
        '実現損益 +¥2,200',
        '',
        '[[stock:FIC-01]]',
      ].join('\n'),
    })
  })

  it('includes purchase facts and remaining shares without a heading template', () => {
    const trade: TradeDetail = {
      id: 'fake-trade-buy',
      strategy_id: 'fake-strategy',
      symbol: 'FIC-02',
      side: 'buy',
      qty: 4,
      price: 250,
      fee: 10,
      date: '2099-03-08',
      source: 'manual',
      note: null,
      created_at: '2099-03-08T00:00:00Z',
      updated_at: '2099-03-08T00:00:00Z',
      remaining_qty: 4,
    }

    expect(createTradeReflectionDraft(trade, null)).toEqual({
      title: 'FIC-02を買った振り返り',
      body: ['2099-03-08 買 4 株 @¥250', '', '[[stock:FIC-02]]'].join('\n'),
    })
  })
})
