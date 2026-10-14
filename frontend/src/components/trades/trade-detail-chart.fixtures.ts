import type { components } from '#lib/api/schema.gen'

export const TRADE_DETAIL_CHART_BARS: components['schemas']['Bar'][] = [
  {
    instrument_id: 'FIC-01',
    timeframe: '1d',
    timestamp: '2099-02-09T00:00:00Z',
    open: 980,
    high: 1020,
    low: 970,
    close: 1000,
    volume: 1200,
  },
  {
    instrument_id: 'FIC-01',
    timeframe: '1d',
    timestamp: '2099-02-10T00:00:00Z',
    open: 1000,
    high: 1030,
    low: 990,
    close: 1010,
    volume: 1400,
  },
  {
    instrument_id: 'FIC-01',
    timeframe: '1d',
    timestamp: '2099-02-11T00:00:00Z',
    open: 1010,
    high: 1020,
    low: 960,
    close: 970,
    volume: 1600,
  },
]
