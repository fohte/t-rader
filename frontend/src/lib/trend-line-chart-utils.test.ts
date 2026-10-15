import { describe, expect, it } from 'vitest'

import type { TrendLinePoint } from '#lib/annotation-chart-utils'
import type { components } from '#lib/api/schema.gen'
import { toChartTimestamp } from '#lib/chart-utils'
import { getVisibleTrendLine } from '#lib/trend-line-chart-utils'

type Bar = components['schemas']['Bar']

describe('getVisibleTrendLine', () => {
  it('clips a trendline to the loaded daily bars and interpolates both prices', () => {
    const firstBar: Bar = {
      instrument_id: 'demo-code',
      timeframe: '1d',
      timestamp: '2030-04-02T06:00:00Z',
      open: 100,
      high: 110,
      low: 90,
      close: 105,
      volume: 1000,
    }
    const secondBar: Bar = {
      ...firstBar,
      timestamp: '2030-04-03T06:00:00Z',
      open: 105,
      high: 115,
      low: 95,
      close: 110,
      volume: 1200,
    }
    const bars = [firstBar, secondBar]
    const start: TrendLinePoint = {
      timestamp: '2030-04-01T06:00:00Z',
      price: 90,
    }
    const end: TrendLinePoint = {
      timestamp: '2030-04-04T06:00:00Z',
      price: 150,
    }

    expect(getVisibleTrendLine(start, end, bars)).toEqual({
      data: [
        { time: toChartTimestamp(firstBar.timestamp), value: 110 },
        { time: toChartTimestamp(secondBar.timestamp), value: 130 },
      ],
      isCollapsed: false,
    })
  })

  it('hides a trendline that does not overlap the loaded bars', () => {
    const bars: Bar[] = [
      {
        instrument_id: 'demo-code',
        timeframe: '1d',
        timestamp: '2030-04-02T06:00:00Z',
        open: 100,
        high: 110,
        low: 90,
        close: 105,
        volume: 1000,
      },
    ]

    expect(
      getVisibleTrendLine(
        { timestamp: '2030-03-01T06:00:00Z', price: 90 },
        { timestamp: '2030-03-02T06:00:00Z', price: 95 },
        bars,
      ),
    ).toBeNull()
  })

  it('marks two dates in one weekly bar as a collapsed segment', () => {
    const weeklyBar: Bar = {
      instrument_id: 'demo-code',
      timeframe: '1w',
      timestamp: '2030-04-01T00:00:00Z',
      open: 100,
      high: 120,
      low: 90,
      close: 110,
      volume: 5000,
    }

    expect(
      getVisibleTrendLine(
        { timestamp: '2030-04-02T06:00:00Z', price: 115 },
        { timestamp: '2030-04-03T06:00:00Z', price: 100 },
        [weeklyBar],
      ),
    ).toEqual({
      data: [
        { time: toChartTimestamp(weeklyBar.timestamp), value: 115 },
        { time: toChartTimestamp(weeklyBar.timestamp), value: 100 },
      ],
      isCollapsed: true,
    })
  })
})
