import { describe, expect, it } from 'vitest'

import type { components } from '#lib/api/schema.gen'
import { resolveNoteChangeEvidence } from '#lib/note-change-evidence'
import type { ParsedNoteChangeReference } from '#lib/note-change-reference'

describe('resolveNoteChangeEvidence', () => {
  it.each([
    {
      name: 'calculates the change from the requested bar field',
      startValue: 100,
      savedValue: 10,
      expected: {
        startValue: 100,
        endValue: 110,
        changePercent: 10,
        matchesSavedValue: true,
      },
    },
    {
      name: 'flags a saved value that differs from the current bars',
      startValue: 100,
      savedValue: 12,
      expected: {
        startValue: 100,
        endValue: 110,
        changePercent: 10,
        matchesSavedValue: false,
      },
    },
    {
      name: 'returns undefined when the starting bar value is zero',
      startValue: 0,
      savedValue: 8,
      expected: undefined,
    },
  ])('$name', ({ startValue, savedValue, expected }) => {
    const bars: components['schemas']['Bar'][] = [
      {
        instrument_id: '0000',
        timeframe: '1d',
        timestamp: '2030-01-01T15:00:00.000Z',
        open: 10,
        high: startValue,
        low: 1,
        close: 500,
        volume: 1000,
      },
      {
        instrument_id: '0000',
        timeframe: '1d',
        timestamp: '2030-01-02T15:00:00.000Z',
        open: 11,
        high: 110,
        low: 2,
        close: 600,
        volume: 2000,
      },
    ]
    const reference: ParsedNoteChangeReference = {
      instrumentId: '0000',
      start: '2030-01-02',
      end: '2030-01-03',
      field: 'high',
    }

    expect(resolveNoteChangeEvidence(bars, reference, savedValue)).toEqual(
      expected,
    )
  })
})
