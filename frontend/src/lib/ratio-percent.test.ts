import { describe, expect, it } from 'vitest'

import { formatRatioPercent, parseRatioPercent } from '#lib/ratio-percent'

describe('parseRatioPercent', () => {
  it.each([
    { name: 'empty', input: '', expected: { ratio: null, error: null } },
    {
      name: 'whitespace-only',
      input: '   ',
      expected: { ratio: null, error: null },
    },
    {
      name: 'typical-value',
      input: '15',
      expected: { ratio: 0.15, error: null },
    },
    { name: 'upper-bound', input: '100', expected: { ratio: 1, error: null } },
    {
      name: 'decimal',
      input: '12.5',
      expected: { ratio: 0.125, error: null },
    },
    {
      name: 'zero',
      input: '0',
      expected: {
        ratio: null,
        error: '0 より大きく 100 以下の値を入力してください',
      },
    },
    {
      name: 'negative',
      input: '-5',
      expected: {
        ratio: null,
        error: '0 より大きく 100 以下の値を入力してください',
      },
    },
    {
      name: 'over-100',
      input: '100.1',
      expected: {
        ratio: null,
        error: '0 より大きく 100 以下の値を入力してください',
      },
    },
    {
      name: 'not-a-number',
      input: 'abc',
      expected: {
        ratio: null,
        error: '0 より大きく 100 以下の値を入力してください',
      },
    },
  ])('$name', ({ input, expected }) => {
    expect(parseRatioPercent(input)).toEqual(expected)
  })
})

describe('formatRatioPercent', () => {
  it.each([
    { name: 'null', input: null, expected: '' },
    { name: 'undefined', input: undefined, expected: '' },
    { name: 'exact', input: 0.15, expected: '15' },
    { name: 'full', input: 1, expected: '100' },
    {
      name: 'strips-floating-point-noise',
      input: 0.07,
      expected: '7',
    },
  ])('$name', ({ input, expected }) => {
    expect(formatRatioPercent(input)).toBe(expected)
  })
})
