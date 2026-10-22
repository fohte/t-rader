import { describe, expect, it } from 'vitest'

import {
  parseDashboardConfig,
  parseDashboardPreviewHash,
} from '#components/dashboard-pages/dashboard-config'

const dashboardYaml = `title: 市場の動き
widgets:
  - title: 値動き
    width: half
    queries:
      movement:
        metric: change
        period: 1d
        by: stock
        universe:
          stocks: [DEMO-A]
    display:
      movement:
        label: Movement
        format: percent
    viz:
      type: toplist
      value: movement
      order: desc
      limit: 5
      bar_scale:
        min: -0.1
        max: 0.1
`

const expectedDashboardConfig = {
  ok: true,
  value: {
    title: '市場の動き',
    widgets: [
      {
        title: '値動き',
        width: 'half',
        definition: {
          queries: {
            movement: {
              metric: 'change',
              period: '1d',
              by: 'stock',
              universe: { stocks: ['DEMO-A'] },
            },
          },
          display: {
            movement: { label: 'Movement', format: 'percent' },
          },
          viz: {
            type: 'toplist',
            value: 'movement',
            order: 'desc',
            limit: 5,
            bar_scale: { min: -0.1, max: 0.1 },
          },
        },
      },
    ],
  },
} as const

const expectedDashboardConfigWithFixedAsOf = {
  ok: true,
  value: {
    title: '市場の動き',
    widgets: [
      {
        title: '値動き',
        width: 'half',
        definition: {
          queries: {
            movement: {
              metric: 'change',
              period: '1d',
              by: 'stock',
              universe: { stocks: ['DEMO-A'] },
            },
          },
          display: {
            movement: { label: 'Movement', format: 'percent' },
          },
          viz: {
            type: 'toplist',
            value: 'movement',
            order: 'desc',
            limit: 5,
            bar_scale: { min: -0.1, max: 0.1 },
          },
          time: { as_of: '2099-01-05' },
        },
      },
    ],
  },
} as const

function encodeBase64Url(text: string): string {
  const bytes = new TextEncoder().encode(text)
  const binary = String.fromCharCode(...bytes)
  return btoa(binary)
    .replaceAll('+', '-')
    .replaceAll('/', '_')
    .replace(/=+$/, '')
}

describe('parseDashboardConfig', () => {
  it('YAML からダッシュボードとウィジェットを読み取る', () => {
    expect(parseDashboardConfig(dashboardYaml)).toEqual(expectedDashboardConfig)
  })

  it('壊れた YAML を構文エラーとして返す', () => {
    expect(parseDashboardConfig('title: [\nwidgets: []')).toEqual({
      ok: false,
      issues: [{ path: '$', message: 'YAML の構文を確認してください。' }],
    })
  })

  it('不正な幅を検証エラーとして返す', () => {
    expect(
      parseDashboardConfig(dashboardYaml.replace('width: half', 'width: wide')),
    ).toEqual({
      ok: false,
      issues: [
        {
          path: 'widgets[0].width',
          message: 'width は half または full を指定してください。',
        },
      ],
    })
  })

  it.each([
    {
      name: '不正なクエリ',
      yaml: dashboardYaml.replace('metric: change', 'metric: 1'),
      issue: {
        path: 'widgets[0]',
        message: 'queries.movement の形式が正しくありません。',
      },
    },
    {
      name: '時間範囲',
      yaml: dashboardYaml.replace(
        '    viz:',
        '    time:\n      range: 1d\n    viz:',
      ),
      issue: {
        path: 'widgets[0].time.range',
        message: 'time.range は line 以外では使えません。',
      },
    },
    {
      name: '未知の基準日項目',
      yaml: dashboardYaml.replace(
        '    viz:',
        "    time:\n      asof: '2099-01-05'\n    viz:",
      ),
      issue: {
        path: 'widgets[0].time.asof',
        message: '未対応の項目です。',
      },
    },
    {
      name: '未知のウィジェット項目',
      yaml: dashboardYaml.replace(
        '    width: half',
        '    width: half\n    formula: {}',
      ),
      issue: {
        path: 'widgets[0].formula',
        message: '未対応の項目です。',
      },
    },
    {
      name: '非対応の viz',
      yaml: dashboardYaml.replace('type: toplist', 'type: heatmap'),
      issue: {
        path: 'widgets[0].viz',
        message:
          'viz.type は treemap、toplist、table または line を指定してください。',
      },
    },
  ])('$name を検証エラーとして返す', ({ yaml, issue }) => {
    expect(parseDashboardConfig(yaml)).toEqual({
      ok: false,
      issues: [issue],
    })
  })

  it('YAML の固定基準日を読み取る', () => {
    expect(
      parseDashboardConfig(
        dashboardYaml.replace(
          '    viz:',
          "    time:\n      as_of: '2099-01-05'\n    viz:",
        ),
      ),
    ).toEqual(expectedDashboardConfigWithFixedAsOf)
  })

  it('table の列順を保持してダッシュボード設定を読み取る', () => {
    const tableYaml = dashboardYaml
      .replace('type: toplist', 'type: table')
      .replace(
        '      value: movement\n      order: desc\n      limit: 5\n      bar_scale:\n        min: -0.1\n        max: 0.1',
        '      columns:\n        - movement',
      )

    expect(parseDashboardConfig(tableYaml)).toEqual({
      ok: true,
      value: {
        title: '市場の動き',
        widgets: [
          {
            title: '値動き',
            width: 'half',
            definition: {
              queries: {
                movement: {
                  metric: 'change',
                  period: '1d',
                  by: 'stock',
                  universe: { stocks: ['DEMO-A'] },
                },
              },
              display: {
                movement: { label: 'Movement', format: 'percent' },
              },
              viz: { type: 'table', columns: ['movement'] },
            },
          },
        ],
      },
    })
  })

  it('期間と系列を含む line widget を読み取る', () => {
    const lineYaml = dashboardYaml
      .replace('type: toplist', 'type: line')
      .replace(
        '      value: movement\n      order: desc\n      limit: 5\n      bar_scale:\n        min: -0.1\n        max: 0.1',
        '      series: [movement]',
      )
      .replace(
        '    viz:',
        '    time:\n      range: 2099-01-01..2099-01-05\n    viz:',
      )

    expect(parseDashboardConfig(lineYaml)).toEqual({
      ok: true,
      value: {
        title: '市場の動き',
        widgets: [
          {
            title: '値動き',
            width: 'half',
            definition: {
              queries: {
                movement: {
                  metric: 'change',
                  period: '1d',
                  by: 'stock',
                  universe: { stocks: ['DEMO-A'] },
                },
              },
              display: {
                movement: { label: 'Movement', format: 'percent' },
              },
              time: { range: '2099-01-01..2099-01-05' },
              viz: { type: 'line', series: ['movement'] },
            },
          },
        ],
      },
    })
  })

  it('プレビュー URL の URL-safe Base64 から設定を読み取る', () => {
    expect(
      parseDashboardPreviewHash(`#${encodeBase64Url(dashboardYaml)}`),
    ).toEqual(expectedDashboardConfig)
  })

  it('プレビュー URL に含まれる期間 line widget を読み取る', () => {
    const lineYaml = dashboardYaml
      .replace('type: toplist', 'type: line')
      .replace(
        '      value: movement\n      order: desc\n      limit: 5\n      bar_scale:\n        min: -0.1\n        max: 0.1',
        '      series: [movement]',
      )
      .replace('    viz:', '    time:\n      range: 1y\n    viz:')

    expect(parseDashboardPreviewHash(`#${encodeBase64Url(lineYaml)}`)).toEqual({
      ok: true,
      value: {
        title: '市場の動き',
        widgets: [
          {
            title: '値動き',
            width: 'half',
            definition: {
              queries: {
                movement: {
                  metric: 'change',
                  period: '1d',
                  by: 'stock',
                  universe: { stocks: ['DEMO-A'] },
                },
              },
              display: {
                movement: { label: 'Movement', format: 'percent' },
              },
              time: { range: '1y' },
              viz: { type: 'line', series: ['movement'] },
            },
          },
        ],
      },
    })
  })

  it('復号できないプレビュー URL を検証エラーとして返す', () => {
    expect(parseDashboardPreviewHash('#%%%')).toEqual({
      ok: false,
      issues: [
        {
          path: '$preview',
          message: 'プレビュー URL の YAML データを読み取れません。',
        },
      ],
    })
  })
})
