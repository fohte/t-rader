import { describe, expect, it } from 'vitest'

import { parseNoteChangeReference } from '#lib/note-change-reference'

describe('parseNoteChangeReference', () => {
  it('returns the instrument, date range, and field from a change token', () => {
    expect(
      parseNoteChangeReference(
        '[[change:US:FICTIONAL-A@2030-01-02..2030-01-03:volume]]',
      ),
    ).toEqual({
      instrumentId: 'US:FICTIONAL-A',
      start: '2030-01-02',
      end: '2030-01-03',
      field: 'volume',
    })
  })
})
