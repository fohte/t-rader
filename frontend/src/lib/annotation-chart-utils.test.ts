import { describe, expect, it } from 'vitest'

import {
  bucketAnnotationPeriod,
  bucketAnnotationTimestamp,
  clipAnnotationBandSegment,
  clusterAnnotationBandMarkers,
  getAnnotationPeriodLineRange,
  groupAnnotationBandTargets,
} from '#lib/annotation-chart-utils'
import type { components } from '#lib/api/schema.gen'

type Bar = components['schemas']['Bar']

describe('bucketAnnotationTimestamp', () => {
  it('maps a JST midnight annotation to the bar for that date', () => {
    const bars: Bar[] = [
      {
        instrument_id: 'DEMO-JP-ALPHA',
        timeframe: '1d',
        timestamp: '2025-01-02T00:00:00.000Z',
        open: 100,
        high: 110,
        low: 90,
        close: 105,
        volume: 1000,
      },
      {
        instrument_id: 'DEMO-JP-ALPHA',
        timeframe: '1d',
        timestamp: '2025-01-03T00:00:00.000Z',
        open: 105,
        high: 115,
        low: 95,
        close: 110,
        volume: 1200,
      },
    ]

    expect(bucketAnnotationTimestamp('2025-01-01T15:00:00.000Z', bars)).toBe(
      1735776000,
    )
  })

  it('maps an intraday annotation to the most recent bar in its interval', () => {
    const bars: Bar[] = [
      {
        instrument_id: 'DEMO-JP-ALPHA',
        timeframe: '5m',
        timestamp: '2025-01-02T10:00:00.000Z',
        open: 100,
        high: 110,
        low: 90,
        close: 105,
        volume: 1000,
      },
      {
        instrument_id: 'DEMO-JP-ALPHA',
        timeframe: '5m',
        timestamp: '2025-01-02T10:05:00.000Z',
        open: 105,
        high: 115,
        low: 95,
        close: 110,
        volume: 1200,
      },
      {
        instrument_id: 'DEMO-JP-ALPHA',
        timeframe: '5m',
        timestamp: '2025-01-02T10:10:00.000Z',
        open: 110,
        high: 120,
        low: 100,
        close: 115,
        volume: 1300,
      },
    ]

    expect(bucketAnnotationTimestamp('2025-01-02T10:07:00.000Z', bars)).toBe(
      1735812300,
    )
  })
})

describe('bucketAnnotationPeriod', () => {
  it('buckets both ends and clips a partially covered daily period to available bars', () => {
    const bars: Bar[] = [
      {
        instrument_id: 'demo-code',
        timeframe: '1d',
        timestamp: '2025-01-02T00:00:00.000Z',
        open: 100,
        high: 110,
        low: 90,
        close: 105,
        volume: 1000,
      },
      {
        instrument_id: 'demo-code',
        timeframe: '1d',
        timestamp: '2025-01-03T00:00:00.000Z',
        open: 105,
        high: 115,
        low: 95,
        close: 110,
        volume: 1200,
      },
    ]

    expect(
      bucketAnnotationPeriod(
        '2025-01-01T12:00:00.000Z',
        '2025-01-02T15:00:00.000Z',
        bars,
      ),
    ).toEqual({ start: 1735776000, end: 1735862400 })
  })

  it('buckets intraday ends to the same visible bars as point annotations', () => {
    const bars: Bar[] = [
      {
        instrument_id: 'demo-code',
        timeframe: '5m',
        timestamp: '2025-01-02T10:00:00.000Z',
        open: 100,
        high: 110,
        low: 90,
        close: 105,
        volume: 1000,
      },
      {
        instrument_id: 'demo-code',
        timeframe: '5m',
        timestamp: '2025-01-02T10:05:00.000Z',
        open: 105,
        high: 115,
        low: 95,
        close: 110,
        volume: 1200,
      },
      {
        instrument_id: 'demo-code',
        timeframe: '5m',
        timestamp: '2025-01-02T10:10:00.000Z',
        open: 110,
        high: 120,
        low: 100,
        close: 115,
        volume: 1300,
      },
    ]

    expect(
      bucketAnnotationPeriod(
        '2025-01-02T09:55:00.000Z',
        '2025-01-02T10:07:00.000Z',
        bars,
      ),
    ).toEqual({ start: 1735812000, end: 1735812300 })
  })
})

describe('getAnnotationPeriodLineRange', () => {
  it('extends a period within one bucket to an adjacent bar', () => {
    const bars: Bar[] = [
      {
        instrument_id: 'demo-code',
        timeframe: '1d',
        timestamp: '2025-04-10T00:00:00.000Z',
        open: 100,
        high: 110,
        low: 90,
        close: 105,
        volume: 1000,
      },
      {
        instrument_id: 'demo-code',
        timeframe: '1d',
        timestamp: '2025-04-11T00:00:00.000Z',
        open: 105,
        high: 115,
        low: 95,
        close: 110,
        volume: 1200,
      },
    ]

    expect(
      getAnnotationPeriodLineRange(
        '2025-04-10T00:00:00.000Z',
        '2025-04-10T12:00:00.000Z',
        bars,
      ),
    ).toEqual({ start: 1744243200, end: 1744329600 })
  })

  it('extends a same-bucket period on the last visible bar to the previous bar', () => {
    const bars: Bar[] = [
      {
        instrument_id: 'demo-code',
        timeframe: '1d',
        timestamp: '2025-04-10T00:00:00.000Z',
        open: 100,
        high: 110,
        low: 90,
        close: 105,
        volume: 1000,
      },
      {
        instrument_id: 'demo-code',
        timeframe: '1d',
        timestamp: '2025-04-11T00:00:00.000Z',
        open: 105,
        high: 115,
        low: 95,
        close: 110,
        volume: 1200,
      },
    ]

    expect(
      getAnnotationPeriodLineRange(
        '2025-04-11T03:00:00.000Z',
        '2025-04-11T12:00:00.000Z',
        bars,
      ),
    ).toEqual({ start: 1744243200, end: 1744329600 })
  })
})

describe('clipAnnotationBandSegment', () => {
  it.each([
    ['clips the left edge', -12, 30, 100, { startX: 0, endX: 30 }] as const,
    ['clips the right edge', 70, 125, 100, { startX: 70, endX: 100 }] as const,
    [
      'keeps a narrow period clickable',
      40,
      40,
      100,
      { startX: 37, endX: 43 },
    ] as const,
    ['omits a segment before the chart', -20, -1, 100, null] as const,
    ['omits a segment after the chart', 101, 120, 100, null] as const,
  ])('%s', (_name, startX, endX, width, expected) => {
    expect(clipAnnotationBandSegment(startX, endX, width)).toEqual(expected)
  })
})

describe('clusterAnnotationBandMarkers', () => {
  it('groups nearby markers and keeps separated markers distinct', () => {
    expect(
      clusterAnnotationBandMarkers([
        { id: 'annotation-a', x: 10 },
        { id: 'annotation-b', x: 18 },
        { id: 'annotation-c', x: 48 },
      ]),
    ).toEqual([
      {
        markers: [
          { id: 'annotation-a', x: 10 },
          { id: 'annotation-b', x: 18 },
        ],
        x: 14,
      },
      {
        markers: [{ id: 'annotation-c', x: 48 }],
        x: 48,
      },
    ])
  })
})

describe('groupAnnotationBandTargets', () => {
  it('keeps a point inside a period as an independent target', () => {
    expect(
      groupAnnotationBandTargets(
        [{ id: 'annotation-point', x: 120 }],
        [{ id: 'annotation-period', startX: 20, endX: 300 }],
      ),
    ).toEqual([
      {
        kind: 'period',
        ids: ['annotation-period'],
        startX: 20,
        endX: 300,
        centerX: 160,
        periods: [{ id: 'annotation-period', startX: 20, endX: 300 }],
      },
      {
        kind: 'marker',
        ids: ['annotation-point'],
        centerX: 120,
        markers: [{ id: 'annotation-point', x: 120 }],
      },
    ])
  })
})
