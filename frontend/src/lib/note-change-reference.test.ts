import { describe, expect, it } from 'vitest'

import { parseNoteChangeReference } from '#lib/note-change-reference'

describe('parseNoteChangeReference', () => {
  it.each([
    {
      name: 'stock changes',
      token: '[[change:US:FICTIONAL-A@2030-01-02..2030-01-03:volume]]',
      expected: {
        kind: 'stock',
        instrumentId: 'US:FICTIONAL-A',
        start: '2030-01-02',
        end: '2030-01-03',
        field: 'volume',
      },
    },
    {
      name: 'indicator changes',
      token: '[[change:indicator:demo-indicator@2030-01-02..2030-01-03]]',
      expected: {
        kind: 'indicator',
        indicatorId: 'demo-indicator',
        start: '2030-01-02',
        end: '2030-01-03',
      },
    },
  ])('parses $name', ({ token, expected }) => {
    expect(parseNoteChangeReference(token)).toEqual(expected)
  })

  it.each([
    {
      name: 'an invalid date',
      token: '[[change:indicator:demo-indicator@2030-02-30..2030-03-01]]',
    },
    {
      name: 'a reversed date range',
      token: '[[change:indicator:demo-indicator@2030-01-03..2030-01-02]]',
    },
    {
      name: 'an indicator token with a stock field suffix',
      token: '[[change:indicator:demo-indicator@2030-01-02..2030-01-03:close]]',
    },
    {
      name: 'an unsupported stock field',
      token: '[[change:fictional-code@2030-01-02..2030-01-03:adjusted]]',
    },
  ])('rejects $name', ({ token }) => {
    expect(parseNoteChangeReference(token)).toBeUndefined()
  })
})
