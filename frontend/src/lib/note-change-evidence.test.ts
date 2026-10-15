import { describe, expect, it } from 'vitest'

import { calculateNoteChangeEvidence } from '#lib/note-change-evidence'

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
})
