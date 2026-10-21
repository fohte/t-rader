import { cleanup, render, screen } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import { MarkdownBody } from '#components/note-detail/markdown-body'
import type { components } from '#lib/api/schema.gen'

type WidgetQueryResponse = components['schemas']['WidgetQueryResponse']

const { useApiQuery } = vi.hoisted(() => ({ useApiQuery: vi.fn() }))

vi.mock('#lib/api/client', () => ({ $api: { useQuery: useApiQuery } }))

vi.mock('#lib/echarts/use-echarts-chart', async () => {
  const React = await import('react')
  const { testEChartsPalette } =
    await import('../../test/echarts-palette.test-helper')
  return {
    useEChartsChart: () => ({
      containerRef: React.useRef<HTMLDivElement>(null),
      palette: testEChartsPalette,
      error: null,
      updateOption: () => undefined,
      dispatchAction: () => undefined,
    }),
  }
})

const VALID_WIDGET_SOURCE = [
  '```widget',
  'queries:',
  '  daily:',
  '    metric: change',
  '    period: 1d',
  '    by: group',
  '    axis: demo-axis',
  '    agg: mean',
  'formulas:',
  '  doubled: daily * 2',
  'viz:',
  '  type: toplist',
  '  value: doubled',
  '  order: desc',
  '  limit: 5',
  'time:',
  '  as_of: 2099-01-05',
  '```',
].join('\n')
const CHANGE_WIDGET_SOURCE = VALID_WIDGET_SOURCE.replace(
  '  value: doubled',
  '  value: daily',
)
const DISPLAY_WIDGET_SOURCE = VALID_WIDGET_SOURCE.replace(
  'viz:',
  'display:\n  daily:\n    label: Daily movement\n    format: percent\nviz:',
).replace('  limit: 5', '  limit: 5\n  bar_scale:\n    min: -0.1\n    max: 0.1')
const TABLE_WIDGET_SOURCE = [
  '\x60\x60\x60widget',
  'queries:',
  '  pbr:',
  '    metric: pbr',
  '    by: stock',
  '    universe:',
  '      stocks: [DEMO-A]',
  '  close:',
  '    metric: close',
  '    by: stock',
  '    universe:',
  '      stocks: [DEMO-A]',
  '  turnover:',
  '    metric: turnover',
  '    by: stock',
  '    universe:',
  '      stocks: [DEMO-A]',
  '  per:',
  '    metric: per',
  '    by: stock',
  '    universe:',
  '      stocks: [DEMO-A]',
  '  indicator_value:',
  '    metric: indicator',
  '    by: none',
  '    indicator: demo-indicator',
  'display:',
  '  pbr:',
  '    label: PBR',
  '    format: number',
  '  close:',
  '    label: 終値',
  '    format: number',
  '  turnover:',
  '    label: 売買代金',
  '    format: number',
  '  per:',
  '    label: PER',
  '    format: number',
  '  indicator_value:',
  '    label: 指標',
  '    format: number',
  'viz:',
  '  type: table',
  '  columns: [pbr, close, turnover, per, indicator_value]',
  'time:',
  '  as_of: 2099-01-05',
  '\x60\x60\x60',
].join('\n')

const WIDGET_SOURCES_WITHOUT_AS_OF = [
  {
    name: 'time is omitted',
    source: VALID_WIDGET_SOURCE.replace('time:\n  as_of: 2099-01-05\n', ''),
  },
  {
    name: 'time.as_of is omitted',
    source: VALID_WIDGET_SOURCE.replace(
      'time:\n  as_of: 2099-01-05',
      'time: {}',
    ),
  },
  {
    name: 'time.as_of is null',
    source: VALID_WIDGET_SOURCE.replace(
      'time:\n  as_of: 2099-01-05',
      'time:\n  as_of: null',
    ),
  },
]

const RESULT: WidgetQueryResponse = {
  as_of: '2099-01-05',
  rows: [
    {
      key: 'group:demo-axis/demo-group',
      name: 'デモグループ',
      values: { doubled: 2.1 },
    },
  ],
}
const TABLE_RESULT: WidgetQueryResponse = {
  as_of: '2099-01-05',
  rows: [
    {
      key: 'stock:DEMO-A',
      name: '架空銘柄 A',
      values: {
        close: 1240,
        turnover: 2400000,
        per: 12.5,
        pbr: 1.3,
        indicator_value: 4.2,
      },
    },
  ],
}

function readWidgetTableOutput(widget: HTMLElement, table: HTMLElement) {
  return {
    asOf: widget.querySelector('time')?.textContent,
    headers: Array.from(table.querySelectorAll('thead th'), (cell) =>
      cell.textContent.trim(),
    ),
    rows: Array.from(table.querySelectorAll('tbody tr'), (row) =>
      Array.from(row.querySelectorAll('th, td'), (cell) =>
        cell.textContent.trim(),
      ),
    ),
  }
}

beforeEach(() => {
  useApiQuery.mockReturnValue({
    data: undefined,
    isPending: true,
    isError: false,
    error: undefined,
  })
})

afterEach(() => {
  cleanup()
  useApiQuery.mockReset()
})

describe('MarkdownBody widget blocks', () => {
  it('sends the parsed definition to the widget query API', () => {
    render(<MarkdownBody source={VALID_WIDGET_SOURCE} />)

    expect(useApiQuery.mock.calls).toEqual([
      [
        'post',
        '/api/widgets/query',
        {
          body: {
            queries: {
              daily: {
                metric: 'change',
                period: '1d',
                by: 'group',
                axis: 'demo-axis',
                agg: 'mean',
              },
            },
            formulas: { doubled: 'daily * 2' },
            viz: { type: 'toplist', value: 'doubled', order: 'desc', limit: 5 },
            time: { as_of: '2099-01-05' },
          },
        },
        { enabled: true },
      ],
    ])
  })

  it('sends display formats and fixed scales to the widget query API', () => {
    render(<MarkdownBody source={DISPLAY_WIDGET_SOURCE} />)

    expect(useApiQuery.mock.calls).toEqual([
      [
        'post',
        '/api/widgets/query',
        {
          body: {
            queries: {
              daily: {
                metric: 'change',
                period: '1d',
                by: 'group',
                axis: 'demo-axis',
                agg: 'mean',
              },
            },
            formulas: { doubled: 'daily * 2' },
            display: { daily: { label: 'Daily movement', format: 'percent' } },
            viz: {
              type: 'toplist',
              value: 'doubled',
              order: 'desc',
              limit: 5,
              bar_scale: { min: -0.1, max: 0.1 },
            },
            time: { as_of: '2099-01-05' },
          },
        },
        { enabled: true },
      ],
    ])
  })

  it('sends additional metrics and table columns to the widget query API', () => {
    render(<MarkdownBody source={TABLE_WIDGET_SOURCE} />)

    expect(useApiQuery.mock.calls).toEqual([
      [
        'post',
        '/api/widgets/query',
        {
          body: {
            queries: {
              pbr: {
                metric: 'pbr',
                by: 'stock',
                universe: { stocks: ['DEMO-A'] },
              },
              close: {
                metric: 'close',
                by: 'stock',
                universe: { stocks: ['DEMO-A'] },
              },
              turnover: {
                metric: 'turnover',
                by: 'stock',
                universe: { stocks: ['DEMO-A'] },
              },
              per: {
                metric: 'per',
                by: 'stock',
                universe: { stocks: ['DEMO-A'] },
              },
              indicator_value: {
                metric: 'indicator',
                by: 'none',
                indicator: 'demo-indicator',
              },
            },
            display: {
              pbr: { label: 'PBR', format: 'number' },
              close: { label: '終値', format: 'number' },
              turnover: { label: '売買代金', format: 'number' },
              per: { label: 'PER', format: 'number' },
              indicator_value: { label: '指標', format: 'number' },
            },
            viz: {
              type: 'table',
              columns: ['pbr', 'close', 'turnover', 'per', 'indicator_value'],
            },
            time: { as_of: '2099-01-05' },
          },
        },
        { enabled: true },
      ],
    ])
  })

  it('renders the toplist with the resolved base date', () => {
    useApiQuery.mockReturnValue({
      data: RESULT,
      isPending: false,
      isError: false,
      error: undefined,
    })
    render(<MarkdownBody source={VALID_WIDGET_SOURCE} />)

    const widget = screen.getByRole('region', { name: 'ノート内ウィジェット' })
    const renderedWidget = [
      widget.textContent.replace(/\s+/g, ' ').trim(),
      widget.closest('pre') === null ? 'outside <pre>' : 'inside <pre>',
    ].join('\n')
    const expectedWidget = [
      'ウィジェット基準日 2099-01-05doubled2099-01-05自動尺度: 0 / 0 / 2.311デモグループdemo-axis/demo-group+2.1順位一覧から詳細を選ぶ (1)1デモグループdemo-axis/demo-group+2.1',
      'outside <pre>',
    ].join('\n')
    expect(renderedWidget).toEqual(expectedWidget)
  })

  it('renders direct change query values as signed percentages', async () => {
    useApiQuery.mockReturnValue({
      data: {
        as_of: '2099-01-05',
        rows: [
          {
            key: 'group:demo-axis/demo-group',
            name: 'デモグループ',
            values: { daily: 0.0042 },
          },
        ],
      },
      isPending: false,
      isError: false,
      error: undefined,
    })
    render(<MarkdownBody source={CHANGE_WIDGET_SOURCE} />)

    expect(await screen.findByRole('img')).toHaveAttribute(
      'aria-label',
      'change の toplist。基準日 2099-01-05。1 件を降順で表示。1 位 デモグループ、demo-axis/demo-group、+0.42%',
    )
  })

  it('renders a table widget with additional metric columns in note content', () => {
    useApiQuery.mockReturnValue({
      data: TABLE_RESULT,
      isPending: false,
      isError: false,
      error: undefined,
    })
    render(<MarkdownBody source={TABLE_WIDGET_SOURCE} />)
    const widget = screen.getByRole('region', { name: 'ノート内ウィジェット' })
    const table = screen.getByRole('table', { name: 'ウィジェットの表' })

    expect(readWidgetTableOutput(widget, table)).toEqual({
      asOf: '基準日 2099-01-05',
      headers: ['対象', 'PBR', '終値', '売買代金', 'PER', '指標'],
      rows: [['架空銘柄 A', '1.3', '1,240', '2,400,000', '12.5', '4.2']],
    })
  })

  it.each(WIDGET_SOURCES_WITHOUT_AS_OF)(
    'disables the request when $name is missing',
    ({ source }) => {
      render(<MarkdownBody source={source} />)

      expect(useApiQuery.mock.calls).toEqual([
        [
          'post',
          '/api/widgets/query',
          {
            body: {
              queries: {},
              viz: {
                type: 'toplist',
                value: 'placeholder',
                order: 'desc',
                limit: 0,
              },
            },
          },
          { enabled: false },
        ],
      ])
    },
  )

  it.each(WIDGET_SOURCES_WITHOUT_AS_OF)(
    'shows a local validation error when $name is missing',
    ({ source }) => {
      render(<MarkdownBody source={source} />)

      expect(
        screen.getByRole('region', { name: 'ノート内ウィジェット' })
          .textContent,
      ).toEqual(
        'ウィジェットウィジェットの定義を表示できません: time.as_of はノート内ウィジェットで必須です。',
      )
    },
  )

  it('shows the API validation error and keeps the configured base date', () => {
    useApiQuery.mockReturnValue({
      data: undefined,
      isPending: false,
      isError: true,
      error: { status: 400, error: 'queries.daily.agg is invalid' },
    })
    render(<MarkdownBody source={VALID_WIDGET_SOURCE} />)

    expect(
      screen.getByRole('region', { name: 'ノート内ウィジェット' }).textContent,
    ).toEqual(
      'ウィジェット基準日 2099-01-05ウィジェットの定義を表示できません: queries.daily.agg is invalid',
    )
  })

  it('shows a request-shape error as a definition error', () => {
    useApiQuery.mockReturnValue({
      data: undefined,
      isPending: false,
      isError: true,
      error: { status: 422, error: 'invalid request body' },
    })
    render(<MarkdownBody source={VALID_WIDGET_SOURCE} />)

    expect(
      screen.getByRole('region', { name: 'ノート内ウィジェット' }).textContent,
    ).toEqual(
      'ウィジェット基準日 2099-01-05ウィジェットの定義を表示できません: invalid request body',
    )
  })

  it('shows an unavailable-data error when the requested date has no data', () => {
    useApiQuery.mockReturnValue({
      data: undefined,
      isPending: false,
      isError: true,
      error: { status: 404, error: 'no data for requested date' },
    })
    render(<MarkdownBody source={VALID_WIDGET_SOURCE} />)

    expect(
      screen.getByRole('region', { name: 'ノート内ウィジェット' }).textContent,
    ).toEqual(
      'ウィジェット基準日 2099-01-05データを取得できませんでした: no data for requested date',
    )
  })
})
