import { cleanup, render, screen } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import { MarkdownBody } from '#components/note-detail/markdown-body'
import type { components } from '#lib/api/schema.gen'

type WidgetQueryResponse = components['schemas']['WidgetQueryResponse']

const { useApiQuery } = vi.hoisted(() => ({ useApiQuery: vi.fn() }))

vi.mock('#lib/api/client', () => ({ $api: { useQuery: useApiQuery } }))

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
    expect(renderedWidget).toEqual(
      'ウィジェット基準日 2099-01-051デモグループ2.1\noutside <pre>',
    )
  })

  it('disables the request when time.as_of is missing', () => {
    const source = VALID_WIDGET_SOURCE.replace('  as_of: 2099-01-05', '')
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
  })

  it('shows a local validation error when time.as_of is missing', () => {
    const source = VALID_WIDGET_SOURCE.replace('  as_of: 2099-01-05', '')
    render(<MarkdownBody source={source} />)

    expect(
      screen.getByRole('region', { name: 'ノート内ウィジェット' }).textContent,
    ).toEqual(
      'ウィジェットウィジェットの定義を表示できません: queries、対応する viz、time.as_of を含む定義が必要です。',
    )
  })

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
