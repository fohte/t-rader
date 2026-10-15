import { describe, expect, it } from 'vitest'

import {
  calculateChartIndicatorLines,
  type ChartIndicatorBar,
} from '#lib/chart-indicator-calculations'

const bars: ChartIndicatorBar<string>[] = [
  { time: '2026-01-01', close: 10 },
  { time: '2026-01-02', close: 12 },
  { time: '2026-01-03', close: 14 },
  { time: '2026-01-04', close: 16 },
  { time: '2026-01-05', close: 18 },
]

describe('calculateChartIndicatorLines', () => {
  it('maps a simple moving average to a main chart line', () => {
    expect(
      calculateChartIndicatorLines(
        { id: 'indicator-sma', kind: 'sma', period: 3, color: '#f97316' },
        bars,
      ),
    ).toEqual([
      {
        variant: 'main',
        points: [
          { time: '2026-01-03', value: 12 },
          { time: '2026-01-04', value: 14 },
          { time: '2026-01-05', value: 16 },
        ],
      },
    ])
  })

  it('maps an exponential moving average to a main chart line', () => {
    const unevenBars: ChartIndicatorBar<string>[] = [
      { time: '2026-01-01', close: 10 },
      { time: '2026-01-02', close: 12 },
      { time: '2026-01-03', close: 14 },
      { time: '2026-01-04', close: 20 },
      { time: '2026-01-05', close: 18 },
    ]

    expect(
      calculateChartIndicatorLines(
        { id: 'indicator-ema', kind: 'ema', period: 3, color: '#38bdf8' },
        unevenBars,
      ),
    ).toEqual([
      {
        variant: 'main',
        points: [
          { time: '2026-01-03', value: 12 },
          { time: '2026-01-04', value: 16 },
          { time: '2026-01-05', value: 17 },
        ],
      },
    ])
  })

  it('maps Bollinger bands to a main line and two band lines', () => {
    expect(
      calculateChartIndicatorLines(
        {
          id: 'indicator-bollinger',
          kind: 'bollinger',
          period: 2,
          deviation: 2,
          color: '#34d399',
        },
        bars,
      ),
    ).toEqual([
      {
        variant: 'main',
        points: [
          { time: '2026-01-02', value: 11 },
          { time: '2026-01-03', value: 13 },
          { time: '2026-01-04', value: 15 },
          { time: '2026-01-05', value: 17 },
        ],
      },
      {
        variant: 'band',
        points: [
          { time: '2026-01-02', value: 13 },
          { time: '2026-01-03', value: 15 },
          { time: '2026-01-04', value: 17 },
          { time: '2026-01-05', value: 19 },
        ],
      },
      {
        variant: 'band',
        points: [
          { time: '2026-01-02', value: 9 },
          { time: '2026-01-03', value: 11 },
          { time: '2026-01-04', value: 13 },
          { time: '2026-01-05', value: 15 },
        ],
      },
    ])
  })

  it('maps Wilder RSI values to a lower-pane line', () => {
    const rsiBars: ChartIndicatorBar<string>[] = [
      { time: '2026-02-01', close: 10 },
      { time: '2026-02-02', close: 11 },
      { time: '2026-02-03', close: 10 },
      { time: '2026-02-04', close: 11 },
      { time: '2026-02-05', close: 10 },
    ]

    expect(
      calculateChartIndicatorLines(
        { id: 'indicator-rsi', kind: 'rsi', period: 2, color: '#38bdf8' },
        rsiBars,
      ),
    ).toEqual([
      {
        variant: 'main',
        points: [
          { time: '2026-02-03', value: 50 },
          { time: '2026-02-04', value: 75 },
          { time: '2026-02-05', value: 37.5 },
        ],
      },
    ])
  })

  it('maps MACD, signal, and histogram values to a lower pane', () => {
    const macdBars: ChartIndicatorBar<string>[] = [
      { time: '2026-03-01', close: 10 },
      { time: '2026-03-02', close: 12 },
      { time: '2026-03-03', close: 14 },
      { time: '2026-03-04', close: 16 },
      { time: '2026-03-05', close: 24 },
      { time: '2026-03-06', close: 30 },
    ]

    expect(
      calculateChartIndicatorLines(
        {
          id: 'indicator-macd',
          kind: 'macd',
          fastPeriod: 1,
          slowPeriod: 2,
          signalPeriod: 3,
          color: '#34d399',
        },
        macdBars,
      ),
    ).toEqual([
      {
        variant: 'main',
        points: [
          { time: '2026-03-02', value: 1 },
          { time: '2026-03-03', value: 1 },
          { time: '2026-03-04', value: 1 },
          { time: '2026-03-05', value: 3 },
          { time: '2026-03-06', value: 3 },
        ],
      },
      {
        variant: 'signal',
        points: [
          { time: '2026-03-04', value: 1 },
          { time: '2026-03-05', value: 2 },
          { time: '2026-03-06', value: 2.5 },
        ],
      },
      {
        variant: 'histogram',
        points: [
          { time: '2026-03-04', value: 0 },
          { time: '2026-03-05', value: 1 },
          { time: '2026-03-06', value: 0.5 },
        ],
      },
    ])
  })

  it('returns no RSI points before a full period of price changes exists', () => {
    expect(
      calculateChartIndicatorLines(
        { id: 'indicator-rsi', kind: 'rsi', period: 5, color: '#38bdf8' },
        bars,
      ),
    ).toEqual([{ variant: 'main', points: [] }])
  })

  it('returns 50 when every price change is zero', () => {
    const flatBars: ChartIndicatorBar<string>[] = [
      { time: '2026-04-01', close: 20 },
      { time: '2026-04-02', close: 20 },
      { time: '2026-04-03', close: 20 },
      { time: '2026-04-04', close: 20 },
    ]

    expect(
      calculateChartIndicatorLines(
        { id: 'indicator-rsi', kind: 'rsi', period: 2, color: '#38bdf8' },
        flatBars,
      ),
    ).toEqual([
      {
        variant: 'main',
        points: [
          { time: '2026-04-03', value: 50 },
          { time: '2026-04-04', value: 50 },
        ],
      },
    ])
  })
})

describe('invalid chart indicator parameters', () => {
  it.each([0, -1, 1.5, Number.NaN, Number.POSITIVE_INFINITY])(
    'returns empty output for invalid period %s',
    (period) => {
      const readOutput = () => ({
        sma: calculateChartIndicatorLines(
          { id: 'indicator-sma', kind: 'sma', period, color: '#f97316' },
          bars,
        ),
        ema: calculateChartIndicatorLines(
          { id: 'indicator-ema', kind: 'ema', period, color: '#38bdf8' },
          bars,
        ),
        bollinger: calculateChartIndicatorLines(
          {
            id: 'indicator-bollinger',
            kind: 'bollinger',
            period,
            deviation: 2,
            color: '#34d399',
          },
          bars,
        ),
        rsi: calculateChartIndicatorLines(
          { id: 'indicator-rsi', kind: 'rsi', period, color: '#f472b6' },
          bars,
        ),
        macd: calculateChartIndicatorLines(
          {
            id: 'indicator-macd',
            kind: 'macd',
            fastPeriod: period,
            slowPeriod: 26,
            signalPeriod: 9,
            color: '#c084fc',
          },
          bars,
        ),
      })

      expect(readOutput()).toEqual({
        sma: [{ variant: 'main', points: [] }],
        ema: [{ variant: 'main', points: [] }],
        bollinger: [
          { variant: 'main', points: [] },
          { variant: 'band', points: [] },
          { variant: 'band', points: [] },
        ],
        rsi: [{ variant: 'main', points: [] }],
        macd: [
          { variant: 'main', points: [] },
          { variant: 'signal', points: [] },
          { variant: 'histogram', points: [] },
        ],
      })
    },
  )

  it.each([-1, Number.NaN, Number.POSITIVE_INFINITY])(
    'returns empty Bollinger bands for invalid deviation multiplier %s',
    (deviationMultiplier) => {
      expect(
        calculateChartIndicatorLines(
          {
            id: 'indicator-bollinger',
            kind: 'bollinger',
            period: 2,
            deviation: deviationMultiplier,
            color: '#34d399',
          },
          bars,
        ),
      ).toEqual([
        { variant: 'main', points: [] },
        { variant: 'band', points: [] },
        { variant: 'band', points: [] },
      ])
    },
  )

  it('returns empty MACD lines when the fast period is not shorter than the slow period', () => {
    expect(
      calculateChartIndicatorLines(
        {
          id: 'indicator-macd',
          kind: 'macd',
          fastPeriod: 3,
          slowPeriod: 3,
          signalPeriod: 2,
          color: '#34d399',
        },
        bars,
      ),
    ).toEqual([
      { variant: 'main', points: [] },
      { variant: 'signal', points: [] },
      { variant: 'histogram', points: [] },
    ])
  })
})
