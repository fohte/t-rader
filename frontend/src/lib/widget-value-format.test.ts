import { describe, expect, it } from 'vitest'

import {
  formatWidgetValue,
  type WidgetQueryDefinitions,
} from '#lib/widget-value-format'

const queries: WidgetQueryDefinitions = {
  movement: { metric: 'change', by: 'stock' },
  capitalization: { metric: 'mkt_cap', by: 'stock' },
}

describe('formatWidgetValue', () => {
  it.each([
    {
      name: 'change values use signed percentage formatting',
      key: 'movement',
      value: 0.0123,
      expected: '+1.23%',
    },
    {
      name: 'sub-percent change values keep two decimal places',
      key: 'movement',
      value: 0.0033,
      expected: '+0.33%',
    },
    {
      name: 'negative change values use signed percentage formatting',
      key: 'movement',
      value: -0.0045,
      expected: '-0.45%',
    },
    {
      name: 'change values rounded to zero omit the negative sign',
      key: 'movement',
      value: -0.00001,
      expected: '0.00%',
    },
    {
      name: 'zero change values omit a sign',
      key: 'movement',
      value: 0,
      expected: '0.00%',
    },
    {
      name: 'non-change query values keep number formatting',
      key: 'capitalization',
      value: 1234.567,
      expected: '1,234.57',
    },
    {
      name: 'formula values keep number formatting',
      key: 'derived-value',
      value: 0.0123,
      expected: '0.01',
    },
  ])('$name', ({ key, value, expected }) => {
    expect(formatWidgetValue(key, value, queries)).toBe(expected)
  })
})
