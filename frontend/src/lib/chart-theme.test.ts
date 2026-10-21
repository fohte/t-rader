import { afterEach, describe, expect, it } from 'vitest'

import { readChartColor } from '#lib/chart-theme'

const chartStyle = document.documentElement.style
const originalChartColor = chartStyle.getPropertyValue('--chart-test-color')

afterEach(() => {
  if (originalChartColor === '') {
    chartStyle.removeProperty('--chart-test-color')
  } else {
    chartStyle.setProperty('--chart-test-color', originalChartColor)
  }
})

describe('readChartColor', () => {
  it.each([
    ['oklch(0 0 0)', 'rgb(0, 0, 0)'],
    ['oklch(0.5 0 0)', 'rgb(99, 99, 99)'],
    ['rgb(12, 34, 56)', 'rgb(12, 34, 56)'],
  ])('reads %s as %s', (color, expected) => {
    chartStyle.setProperty('--chart-test-color', color)
    expect(readChartColor('--chart-test-color')).toEqual(expected)
  })
})
