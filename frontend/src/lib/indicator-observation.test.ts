import { describe, expect, it } from 'vitest'

import {
  getIndicatorObservationSummary,
  type IndicatorObservation,
  selectIndicatorObservations,
} from '#lib/indicator-observation'

const observations: IndicatorObservation[] = [
  { date: '2026-01-01', value: 100 },
  { date: '2026-09-13', value: 150 },
  { date: '2026-09-14', value: 151.52 },
  { date: '2026-10-14', value: 152.34 },
]

describe('selectIndicatorObservations', () => {
  it('includes the period boundary and excludes older observations', () => {
    expect(
      selectIndicatorObservations(
        observations,
        '1m',
        new Date('2026-10-14T12:00:00Z'),
      ),
    ).toEqual([
      { date: '2026-09-14', value: 151.52 },
      { date: '2026-10-14', value: 152.34 },
    ])
  })
})

describe('getIndicatorObservationSummary', () => {
  it('returns the latest value and its change from the preceding observation', () => {
    expect(
      getIndicatorObservationSummary([
        { date: '2026-10-13', value: 100 },
        { date: '2026-10-14', value: 105 },
      ]),
    ).toEqual({
      latest: { date: '2026-10-14', value: 105 },
      previous: { date: '2026-10-13', value: 100 },
      change: 5,
      changePercent: 5,
    })
  })
})
