import type { components } from '#lib/api/schema.gen'

export const CHANGE_REFERENCE_BARS: components['schemas']['Bar'][] = [
  {
    instrument_id: 'fictional-code',
    timeframe: '1d',
    timestamp: '2030-01-02T00:00:00Z',
    open: 120,
    high: 128,
    low: 118,
    close: 125,
    volume: 1200,
  },
  {
    instrument_id: 'fictional-code',
    timeframe: '1d',
    timestamp: '2030-01-03T00:00:00Z',
    open: 125,
    high: 127,
    low: 114,
    close: 116,
    volume: 1500,
  },
]
