import { describe, expect, it } from 'vitest'

import type { components } from '#lib/api/schema.gen'
import {
  getChartCurrency,
  getChartCurrencyForStockId,
  toTradeMarkers,
  type TradeMarker,
} from '#lib/chart-utils'

type Bar = components['schemas']['Bar']

const firstBar: Bar = {
  close: 101,
  high: 102,
  instrument_id: 'demo-code',
  low: 99,
  open: 100,
  timeframe: '5m',
  timestamp: '2025-04-10T00:05:00.000Z',
  volume: 42,
}

function makeTradeMarker(overrides: Partial<TradeMarker> = {}): TradeMarker {
  return {
    id: '00000000-0000-0000-0000-000000000101',
    side: 'buy',
    date: '2025-04-10',
    ...overrides,
  }
}

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

describe('getChartCurrencyForStockId', () => {
  it.each([
    ['unprefixed domestic stock id', 'FIC-01', '架空市場', 'JPY'],
    ['US stock id', 'US:DEMO-A', undefined, 'USD'],
    ['unsupported foreign market', 'KR:DEMO-A', '架空市場', undefined],
  ])('uses the supported currency for %s', (_, stockId, market, expected) => {
    expect(getChartCurrencyForStockId(stockId, market)).toBe(expected)
  })
})

describe('toTradeMarkers', () => {
  it('maps date-only trades to each date’s first bar and drops dates without bars', () => {
    const secondBar: Bar = {
      ...firstBar,
      timestamp: '2025-04-10T00:10:00.000Z',
    }
    const nextDateBar: Bar = {
      ...firstBar,
      timestamp: '2025-04-11T00:05:00.000Z',
    }

    expect(
      toTradeMarkers(
        [
          makeTradeMarker({
            id: '00000000-0000-0000-0000-000000000102',
            side: 'sell',
            date: '2025-04-11',
          }),
          makeTradeMarker({ date: '2025-04-10' }),
          makeTradeMarker({
            id: '00000000-0000-0000-0000-000000000103',
            side: 'sell',
            date: '2025-04-10',
          }),
          makeTradeMarker({ date: '2025-04-12' }),
        ],
        [firstBar, secondBar, nextDateBar],
      ),
    ).toEqual([
      {
        id: '00000000-0000-0000-0000-000000000101',
        time: 1744243500,
        position: 'belowBar',
        color: '#26a69a',
        shape: 'arrowUp',
      },
      {
        id: '00000000-0000-0000-0000-000000000103',
        time: 1744243500,
        position: 'aboveBar',
        color: '#ef5350',
        shape: 'arrowDown',
      },
      {
        id: '00000000-0000-0000-0000-000000000102',
        time: 1744329900,
        position: 'aboveBar',
        color: '#ef5350',
        shape: 'arrowDown',
      },
    ])
  })

  it('marks the selected trade and sorts it after other trades on the same day', () => {
    expect(
      toTradeMarkers(
        [
          makeTradeMarker({
            id: 'fake-trade-selected',
            date: '2025-04-10',
          }),
          makeTradeMarker({
            id: 'fake-trade-other',
            side: 'sell',
            date: '2025-04-10',
          }),
        ],
        [firstBar],
        'fake-trade-selected',
      ),
    ).toEqual([
      {
        id: 'fake-trade-other',
        time: 1744243500,
        position: 'aboveBar',
        color: '#ef5350',
        shape: 'arrowDown',
      },
      {
        id: 'fake-trade-selected',
        time: 1744243500,
        position: 'belowBar',
        color: '#26a69a',
        shape: 'arrowUp',
        text: '対象',
        size: 2,
      },
    ])
  })
})
