import { describe, expect, it } from 'vitest'

import {
  parseNoteIndicatorObservationReference,
  parseNoteValuationReference,
} from '#lib/note-valuation-indicator-reference'

describe('parseNoteValuationReference', () => {
  it.each([
    { name: 'PER', field: 'per' },
    { name: 'forward PER', field: 'fwd_per' },
    { name: 'PBR', field: 'pbr' },
    { name: 'ROE', field: 'roe' },
    { name: 'forward ROE', field: 'fwd_roe' },
    { name: 'market capitalization', field: 'mkt_cap' },
    { name: 'EPS', field: 'eps' },
    { name: 'forward EPS', field: 'fwd_eps' },
    { name: 'BPS', field: 'bps' },
  ])('reads the $name token', ({ field }) => {
    expect(
      parseNoteValuationReference(
        `[[valuation:US:DEMO-A@2030-01-02:${field}]]`,
      ),
    ).toEqual({
      instrumentId: 'US:DEMO-A',
      date: '2030-01-02',
      field,
    })
  })

  it.each([
    {
      name: 'an unsupported field',
      token: '[[valuation:demo-code@2030-01-02:unknown]]',
    },
    {
      name: 'an impossible date',
      token: '[[valuation:demo-code@2030-02-30:per]]',
    },
    {
      name: 'an empty instrument ID',
      token: '[[valuation:@2030-01-02:per]]',
    },
  ])('rejects $name', ({ token }) => {
    expect(parseNoteValuationReference(token)).toBeUndefined()
  })
})

describe('parseNoteIndicatorObservationReference', () => {
  it('reads a dated indicator token and preserves colons in its ID', () => {
    expect(
      parseNoteIndicatorObservationReference(
        '[[indicator:macro:demo-index@2030-01-02]]',
      ),
    ).toEqual({ indicatorId: 'macro:demo-index', date: '2030-01-02' })
  })

  it.each([
    {
      name: 'an undated indicator token',
      token: '[[indicator:demo-indicator]]',
    },
    {
      name: 'an impossible date',
      token: '[[indicator:demo-indicator@2030-02-30]]',
    },
    {
      name: 'an empty indicator ID',
      token: '[[indicator:@2030-01-02]]',
    },
  ])('rejects $name', ({ token }) => {
    expect(parseNoteIndicatorObservationReference(token)).toBeUndefined()
  })
})
