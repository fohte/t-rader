import { describe, expect, it } from 'vitest'

import { parseGroupRatioDrafts } from '#lib/group-ratio-policy'

describe('parseGroupRatioDrafts', () => {
  it.each([
    {
      name: 'converts percentage inputs for multiple axes',
      rows: [
        { axis: 'example-axis-a', ratioPercent: '25' },
        { axis: 'example-axis-b', ratioPercent: '7.5' },
      ],
      expected: {
        maxGroupRatios: [
          { axis: 'example-axis-a', ratio: 0.25 },
          { axis: 'example-axis-b', ratio: 0.075 },
        ],
        errors: [null, null],
      },
    },
    {
      name: 'rejects a missing axis or limit',
      rows: [
        { axis: '', ratioPercent: '25' },
        { axis: 'example-axis-a', ratioPercent: '' },
      ],
      expected: {
        maxGroupRatios: [],
        errors: [
          { axis: '分類軸を選択してください' },
          { ratioPercent: '上限比率を入力してください' },
        ],
      },
    },
    {
      name: 'rejects repeated axis limits',
      rows: [
        { axis: 'example-axis-a', ratioPercent: '25' },
        { axis: 'example-axis-a', ratioPercent: '30' },
      ],
      expected: {
        maxGroupRatios: [{ axis: 'example-axis-a', ratio: 0.25 }],
        errors: [null, { axis: '分類軸は重複して設定できません' }],
      },
    },
  ])('$name', ({ rows, expected }) => {
    expect(parseGroupRatioDrafts(rows)).toEqual(expected)
  })
})
