import { describe, expect, it } from 'vitest'

import type { IndicatorObservation } from '#lib/indicator-observation'
import { resolveNoteIndicatorChangeEvidence } from '#lib/note-indicator-change-evidence'

const start = '2030-01-02'
const end = '2030-01-03'

describe('resolveNoteIndicatorChangeEvidence', () => {
  it.each([
    {
      name: 'calculates the change from the requested observations',
      startObservations: [{ date: start, value: 100 }],
      endObservations: [{ date: end, value: 112.5 }],
      expected: {
        status: 'ready',
        evidence: {
          startValue: 100,
          endValue: 112.5,
          changePercent: 12.5,
        },
      },
    },
    {
      name: 'reports the date with a missing observation',
      startObservations: [{ date: start, value: 100 }],
      endObservations: [],
      expected: { status: 'missing', missingDates: [end] },
    },
    {
      name: 'preserves both values when the starting observation is zero',
      startObservations: [{ date: start, value: 0 }],
      endObservations: [{ date: end, value: 112.5 }],
      expected: {
        status: 'zero-start',
        startValue: 0,
        endValue: 112.5,
      },
    },
  ])('$name', ({ startObservations, endObservations, expected }) => {
    expect(
      resolveNoteIndicatorChangeEvidence(
        startObservations as IndicatorObservation[],
        endObservations as IndicatorObservation[],
        start,
        end,
      ),
    ).toEqual(expected)
  })
})
