import { describe, expect, it } from 'vitest'

import { oklchToRgb } from '#lib/chart-theme'

describe('oklchToRgb', () => {
  it.each([
    ['oklch(0 0 0)', 'rgb(0, 0, 0)'],
    ['oklch(0.5 0 0)', 'rgb(99, 99, 99)'],
    ['rgb(12, 34, 56)', null],
  ])('converts %s to an sRGB color', (color, expected) => {
    expect(oklchToRgb(color)).toEqual(expected)
  })
})
