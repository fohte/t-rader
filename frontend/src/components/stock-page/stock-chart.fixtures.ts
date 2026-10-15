import type { components } from '#lib/api/schema.gen'

type Bar = components['schemas']['Bar']

const lowerPaneIndicatorCloses = [
  3000, 3020, 3040, 3015, 3055, 3025, 3065, 3030, 3070, 3040, 3080, 3045,
]

export const lowerPaneIndicatorBars: Bar[] = lowerPaneIndicatorCloses.map(
  (close, index) => {
    const open = lowerPaneIndicatorCloses[index - 1] ?? close - 8
    return {
      instrument_id: 'demo-code',
      timeframe: '1d',
      timestamp: new Date(Date.UTC(2030, 3, index + 1, 6)).toISOString(),
      open,
      high: Math.max(open, close) + 16,
      low: Math.min(open, close) - 18,
      close,
      volume: 10000 + index * 100,
    }
  },
)
