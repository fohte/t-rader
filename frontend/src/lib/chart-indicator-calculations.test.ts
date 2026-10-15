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
      })

      expect(readOutput()).toEqual({
        sma: [{ variant: 'main', points: [] }],
        ema: [{ variant: 'main', points: [] }],
        bollinger: [
          { variant: 'main', points: [] },
          { variant: 'band', points: [] },
          { variant: 'band', points: [] },
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
})
