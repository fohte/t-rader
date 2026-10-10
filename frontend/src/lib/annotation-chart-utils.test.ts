import { describe, expect, it } from 'vitest'

import {
  bucketAnnotationTimestamp,
  clusterAnnotationBandMarkers,
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
