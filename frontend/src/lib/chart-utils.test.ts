import { describe, expect, it } from 'vitest'

import type { components } from '#lib/api/schema.gen'
import { getChartCurrency, toTradeMarkers } from '#lib/chart-utils'

type Bar = components['schemas']['Bar']
type Trade = components['schemas']['TradeListItem']

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

function makeTrade(overrides: Partial<Trade> = {}): Trade {
  return {
    id: '00000000-0000-0000-0000-000000000101',
    strategy_id: '00000000-0000-0000-0000-000000000001',
    symbol: 'demo-code',
    side: 'buy',
    qty: 1,
    price: 100,
    fee: 0,
    date: '2025-04-10',
    source: 'manual',
    note: null,
    note_count: 0,
    created_at: '2025-04-10T00:00:00.000Z',
    updated_at: '2025-04-10T00:00:00.000Z',
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
          makeTrade({
            id: '00000000-0000-0000-0000-000000000102',
            side: 'sell',
            date: '2025-04-11',
            price: 105,
          }),
          makeTrade({ date: '2025-04-10', price: 101 }),
          makeTrade({ date: '2025-04-12' }),
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
        id: '00000000-0000-0000-0000-000000000102',
        time: 1744329900,
        position: 'aboveBar',
        color: '#ef5350',
        shape: 'arrowDown',
      },
    ])
  })
})
