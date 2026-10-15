import { describe, expect, it } from 'vitest'

import type { components } from '#lib/api/schema.gen'
import {
  calculateNoteChangeEvidence,
  resolveNoteChangeEvidence,
} from '#lib/note-change-evidence'
import type { ParsedNoteChangeReference } from '#lib/note-change-reference'

describe('calculateNoteChangeEvidence', () => {
  it.each([
    {
      name: 'matches the saved value at the displayed precision',
      savedValue: 8.6199,
      expected: {
        startValue: 2900,
        endValue: 3150,
        changePercent: (250 / 2900) * 100,
        matchesSavedValue: true,
      },
    },
    {
      name: 'flags a saved value that differs from the current bars',
      savedValue: 7.5,
      expected: {
        startValue: 2900,
        endValue: 3150,
        changePercent: (250 / 2900) * 100,
        matchesSavedValue: false,
      },
    },
  ])('$name', ({ savedValue, expected }) => {
    expect(calculateNoteChangeEvidence(2900, 3150, savedValue)).toEqual(
      expected,
    )
  })

  it('does not calculate a percentage from a zero starting value', () => {
    expect(calculateNoteChangeEvidence(0, 3150, 8)).toBeUndefined()
  })

  it('uses the requested field on bars matching the Japan trading dates', () => {
    const bars: components['schemas']['Bar'][] = [
      {
        instrument_id: '0000',
        timeframe: '1d',
        timestamp: '2030-01-01T15:00:00.000Z',
        open: 10,
        high: 100,
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

    expect(resolveNoteChangeEvidence(bars, reference, 10)).toEqual({
      startValue: 100,
      endValue: 110,
      changePercent: 10,
      matchesSavedValue: true,
    })
  })
})
