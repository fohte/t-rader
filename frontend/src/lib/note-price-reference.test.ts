import { describe, expect, it } from 'vitest'

import { parseNotePriceReference } from '#lib/note-price-reference'

describe('parseNotePriceReference', () => {
  it.each([
    {
      name: 'parses an ordinary stock reference',
      token: '[[price:fictional-code@2030-01-02:close]]',
      expected: {
        instrumentId: 'fictional-code',
        date: '2030-01-02',
        field: 'close',
      },
    },
    {
      name: 'preserves a colon in a foreign instrument id',
      token: '[[price:US:FICTIONAL-A@2030-01-02:open]]',
      expected: {
        instrumentId: 'US:FICTIONAL-A',
        date: '2030-01-02',
        field: 'open',
      },
    },
  ])('$name', ({ token, expected }) => {
    expect(parseNotePriceReference(token)).toEqual(expected)
  })

  it('rejects a reference with an impossible date', () => {
    expect(
      parseNotePriceReference('[[price:fictional-code@2030-02-30:close]]'),
    ).toBeUndefined()
  })
})
