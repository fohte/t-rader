import { describe, expect, it } from 'vitest'

import {
  getInitialDashboardDate,
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
    viz:
      type: toplist
      value: movement
      order: desc
      limit: 5
    time:
      as_of: '2099-01-05'
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
          viz: {
            type: 'toplist',
            value: 'movement',
            order: 'desc',
            limit: 5,
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
      name: '未対応の指標',
      yaml: dashboardYaml.replace('metric: change', 'metric: close'),
    },
    {
      name: '未対応の期間',
      yaml: dashboardYaml.replace(
        "      as_of: '2099-01-05'",
        "      as_of: '2099-01-05'\n      range: 1d",
      ),
    },
  ])('$name を検証エラーとして返す', ({ yaml }) => {
    expect(parseDashboardConfig(yaml)).toEqual({
      ok: false,
      issues: [
        {
          path: 'widgets[0]',
          message:
            'queries と treemap または toplist の viz を含むウィジェット定義が必要です。',
        },
      ],
    })
  })

  it('プレビュー URL の URL-safe Base64 から設定を読み取る', () => {
    expect(
      parseDashboardPreviewHash(`#${encodeBase64Url(dashboardYaml)}`),
    ).toEqual(expectedDashboardConfig)
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

  it('同じ基準日のウィジェットにはその日付を使う', () => {
    const parsed = parseDashboardConfig(dashboardYaml)
    expect(
      parsed.ok ? getInitialDashboardDate(parsed.value, '2099-02-01') : null,
    ).toEqual('2099-01-05')
  })

  it('異なる基準日のウィジェットには一括基準日を表示しない', () => {
    const parsed = parseDashboardConfig(
      `${dashboardYaml}  - title: 時価総額
    width: half
    queries:
      capitalization:
        metric: mkt_cap
        by: stock
        universe:
          stocks: [DEMO-B]
    viz:
      type: toplist
      value: capitalization
      order: desc
      limit: 5
    time:
      as_of: '2098-12-01'
`,
    )
    expect(
      parsed.ok ? getInitialDashboardDate(parsed.value, '2099-02-01') : null,
    ).toEqual(null)
  })

  it('基準日を持つウィジェットがなければ fallback を返す', () => {
    expect(
      getInitialDashboardDate({ title: '空', widgets: [] }, '2099-02-01'),
    ).toEqual('2099-02-01')
  })
})
