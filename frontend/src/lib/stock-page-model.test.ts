import { describe, expect, it } from 'vitest'

import type { components } from '#lib/api/schema.gen'
import { buildStockQuote, sortTradesNewestFirst } from '#lib/stock-page-model'

type Bar = components['schemas']['Bar']

const bars: Bar[] = [
  {
    instrument_id: 'demo-code',
    timeframe: '1d',
    timestamp: '2030-04-02T06:00:00Z',
    open: 118,
    high: 122,
    low: 117,
    close: 120,
    volume: 1000,
  },
  {
    instrument_id: 'demo-code',
    timeframe: '1d',
    timestamp: '2030-04-03T06:00:00Z',
    open: 121,
    high: 127,
    low: 120,
    close: 125,
    volume: 1300,
  },
]

describe('buildStockQuote', () => {
  it('returns the latest close and its change from the previous daily bar', () => {
    expect(buildStockQuote(bars)).toEqual({
      close: 125,
      change: 5,
      changePercent: 4.17,
      timestamp: '2030-04-03T06:00:00Z',
    })
  })

  it('returns null when no daily bars are available', () => {
    expect(buildStockQuote([])).toBeNull()
  })
})

describe('sortTradesNewestFirst', () => {
  it('orders trades by date from newest to oldest', () => {
    expect(
      sortTradesNewestFirst([
        {
          id: 'demo-trade-older',
          strategy_id: '00000000-0000-0000-0000-000000000001',
          symbol: 'demo-code',
          side: 'buy',
          qty: 10,
          price: 120,
          fee: 0,
          date: '2030-04-01',
          source: 'manual',
          note: null,
          note_count: 0,
          created_at: '2030-04-01T06:00:00Z',
          updated_at: '2030-04-01T06:00:00Z',
        },
        {
          id: 'demo-trade-newer',
          strategy_id: '00000000-0000-0000-0000-000000000001',
          symbol: 'demo-code',
          side: 'sell',
          qty: 5,
          price: 125,
          fee: 0,
          date: '2030-04-03',
          source: 'manual',
          note: null,
          note_count: 0,
          created_at: '2030-04-03T06:00:00Z',
          updated_at: '2030-04-03T06:00:00Z',
        },
      ]),
    ).toEqual([
      {
        id: 'demo-trade-newer',
        strategy_id: '00000000-0000-0000-0000-000000000001',
        symbol: 'demo-code',
        side: 'sell',
        qty: 5,
        price: 125,
        fee: 0,
        date: '2030-04-03',
        source: 'manual',
        note: null,
        note_count: 0,
        created_at: '2030-04-03T06:00:00Z',
        updated_at: '2030-04-03T06:00:00Z',
      },
      {
        id: 'demo-trade-older',
        strategy_id: '00000000-0000-0000-0000-000000000001',
        symbol: 'demo-code',
        side: 'buy',
        qty: 10,
        price: 120,
        fee: 0,
        date: '2030-04-01',
        source: 'manual',
        note: null,
        note_count: 0,
        created_at: '2030-04-01T06:00:00Z',
        updated_at: '2030-04-01T06:00:00Z',
      },
    ])
  })
})
