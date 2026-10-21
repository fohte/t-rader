import { RouterProvider } from '@tanstack/react-router'
import { act, cleanup, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import type { ECElementEvent } from 'echarts/core'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { WidgetTreemap } from '#components/widgets/widget-treemap'
import type { components } from '#lib/api/schema.gen'
import type { EChartsOptionFactory } from '#lib/echarts/use-echarts-chart'
import type { WidgetQueryDefinitions } from '#lib/widget-value-format'
import { createStoryRouter } from '#storybook/story-router'

type ChartClickHandler = (event: ECElementEvent) => void

const chartHarness = vi.hoisted(() => ({
  clickHandler: null as ChartClickHandler | null,
  actions: [] as unknown[],
  options: [] as unknown[],
}))

vi.mock('#lib/echarts/use-echarts-chart', async () => {
  const React = await import('react')
  const { testEChartsPalette } =
    await import('../../test/echarts-palette.test-helper')
  return {
    useEChartsChart: ({
      createOption,
      onClick,
    }: {
      createOption: EChartsOptionFactory
      onClick: ChartClickHandler
    }) => {
      chartHarness.clickHandler = onClick
      React.useEffect(() => {
        chartHarness.options.push(createOption(testEChartsPalette))
      }, [createOption])
      return {
        containerRef: React.useRef<HTMLDivElement>(null),
        palette: testEChartsPalette,
        error: null,
        updateOption: () =>
          chartHarness.options.push(createOption(testEChartsPalette)),
        dispatchAction: (action: unknown) => chartHarness.actions.push(action),
      }
    },
  }
})

type WidgetQueryResponse = components['schemas']['WidgetQueryResponse']
type WidgetTreemapDefinition = Extract<
  components['schemas']['WidgetVisualizationRequest'],
  { type: 'treemap' }
>

const viz: WidgetTreemapDefinition = {
  type: 'treemap',
  size: 'size',
  color: 'change',
}
const hierarchyViz: WidgetTreemapDefinition = {
  ...viz,
  hierarchy: { axis: 'demo-axis' },
}
const queries: WidgetQueryDefinitions = {
  change: { metric: 'change', by: 'stock' },
}
const hierarchyResult: WidgetQueryResponse = {
  as_of: '2099-01-01',
  rows: [
    {
      key: 'stock:DEMO-A',
      name: '架空銘柄 A',
      values: { size: 9, change: 0.015 },
      classification: {
        axis: 'demo-axis',
        group: { key: 'demo-group-a', name: '架空分類 A' },
      },
    },
    {
      key: 'stock:DEMO-B',
      name: '架空銘柄 B',
      values: { size: 1, change: -0.008 },
      classification: {
        axis: 'demo-axis',
        group: { key: 'demo-group-a', name: '架空分類 A' },
      },
    },
    {
      key: 'stock:DEMO-C',
      name: '架空銘柄 C',
      values: { size: 5, change: 0 },
      classification: {
        axis: 'demo-axis',
        group: { key: 'demo-group-b', name: '架空分類 B' },
      },
    },
  ],
}

function resetChartHarness() {
  chartHarness.clickHandler = null
  chartHarness.actions = []
  chartHarness.options = []
}

function emitChartClick(data: Record<string, string>) {
  chartHarness.clickHandler?.({
    type: 'click',
    componentType: 'series',
    componentSubType: 'treemap',
    componentIndex: 0,
    name: '',
    dataIndex: 0,
    data,
    value: 0,
    $vars: [],
  })
}

function getLastOptionNodeIds(): string[] {
  const option = chartHarness.options.at(-1)
  if (!isRecord(option)) return []
  const series = Array.isArray(option.series) ? option.series : [option.series]
  const firstSeries = series[0]
  if (!isRecord(firstSeries) || !Array.isArray(firstSeries.data)) return []
  return firstSeries.data.flatMap((node) =>
    isRecord(node) && typeof node.id === 'string' ? [node.id] : [],
  )
}

function getLastOptionLeafColors(): string[] {
  const option = chartHarness.options.at(-1)
  if (!isRecord(option)) return []
  const series = Array.isArray(option.series) ? option.series : [option.series]
  const firstSeries = series[0]
  if (!isRecord(firstSeries) || !Array.isArray(firstSeries.data)) return []
  const rows = firstSeries.data.flatMap((node) => {
    if (!isRecord(node)) return []
    if (!Array.isArray(node.children)) return [node]
    const children: unknown[] = node.children
    return children
  })
  return rows.flatMap((row) => {
    if (!isRecord(row) || !isRecord(row.itemStyle)) return []
    return typeof row.itemStyle.color === 'string' ? [row.itemStyle.color] : []
  })
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null
}

function renderWidget(
  result: WidgetQueryResponse,
  widgetViz: WidgetTreemapDefinition = viz,
  props: { initialFocusGroupKey?: string; initialSelectedRowKey?: string } = {},
) {
  const router = createStoryRouter(
    () => (
      <WidgetTreemap
        {...props}
        queries={queries}
        result={result}
        viz={widgetViz}
      />
    ),
    { paths: ['/groups/$axisKey/$groupKey', '/stocks/$stockId'] },
  )
  const rendered = render(<RouterProvider router={router} />)
  return screen
    .findByRole('navigation', { name: '表示中の階層' })
    .then(() => rendered)
}

afterEach(() => {
  cleanup()
  resetChartHarness()
})

describe('WidgetTreemap', () => {
  it('分類見出しからフォーカスし、全体表示で階層を戻す', async () => {
    const user = userEvent.setup()
    await renderWidget(hierarchyResult, hierarchyViz)

    act(() => {
      emitChartClick({
        id: 'sector:["demo-axis","demo-group-a"]',
        kind: 'group',
      })
    })
    const focusBreadcrumb = screen.getByRole('navigation', {
      name: '表示中の階層',
    }).textContent
    const focusActions = chartHarness.actions
    const focusColors = getLastOptionLeafColors()

    await user.click(screen.getByRole('button', { name: '全体を表示' }))

    function readInteractionOutput() {
      return {
        focused: {
          breadcrumb: focusBreadcrumb,
          actions: focusActions,
          colors: focusColors,
        },
        overview: {
          breadcrumb: screen.getByRole('navigation', { name: '表示中の階層' })
            .textContent,
          topLevelIds: getLastOptionNodeIds(),
          colors: getLastOptionLeafColors(),
        },
      }
    }

    expect(readInteractionOutput()).toEqual({
      focused: {
        breadcrumb: 'すべての分類/架空分類 A',
        actions: [
          {
            type: 'treemapRootToNode',
            seriesId: 'widget-treemap',
            targetNodeId: 'sector:["demo-axis","demo-group-a"]',
          },
        ],
        colors: ['#ef5350', '#2b5c7a', '#1a1a1a'],
      },
      overview: {
        breadcrumb: 'すべての分類',
        topLevelIds: [
          'sector:["demo-axis","demo-group-a"]',
          'sector:["demo-axis","demo-group-b"]',
        ],
        colors: ['#ef5350', '#2b5c7a', '#1a1a1a'],
      },
    })
  })

  it('銘柄をクリックすると詳細とアプリ内リンクを表示する', async () => {
    await renderWidget(hierarchyResult, hierarchyViz)

    act(() => {
      emitChartClick({ kind: 'row', rowKey: 'stock:DEMO-A' })
    })

    const detail = await screen.findByRole('region', {
      name: '選択した行の詳細',
    })
    function readDetailOutput() {
      return {
        title: detail.querySelector('h3')?.textContent,
        size: detail.querySelector('dd')?.textContent,
        href: screen
          .getByRole('link', { name: '銘柄ページを開く' })
          .getAttribute('href'),
      }
    }

    expect(readDetailOutput()).toEqual({
      title: '架空銘柄 A',
      size: '9',
      href: '/stocks/DEMO-A',
    })
  })

  it('全件一覧の行をキーボードで選択できる', async () => {
    const user = userEvent.setup()
    await renderWidget(hierarchyResult, hierarchyViz)
    await user.click(screen.getByText(/全件一覧/))

    const row = screen.getByRole('button', {
      name: '架空銘柄 B, change: -0.80%, 詳細を表示',
    })
    row.focus()
    await user.keyboard('{Enter}')

    const detail = await screen.findByRole('region', {
      name: '選択した行の詳細',
    })
    function readDetailOutput() {
      return {
        title: detail.querySelector('h3')?.textContent,
        href: screen
          .getByRole('link', { name: '銘柄ページを開く' })
          .getAttribute('href'),
      }
    }

    expect(readDetailOutput()).toEqual({
      title: '架空銘柄 B',
      href: '/stocks/DEMO-B',
    })
  })

  it('全件一覧の行をタッチポインターで選択できる', async () => {
    const user = userEvent.setup()
    await renderWidget(hierarchyResult, hierarchyViz)
    await user.click(screen.getByText(/全件一覧/))
    const row = screen.getByRole('button', {
      name: '架空銘柄 C, change: 0.00%, 詳細を表示',
    })

    await user.pointer([
      { keys: '[TouchA>]', target: row },
      { keys: '[/TouchA]', target: row },
    ])

    const detail = await screen.findByRole('region', {
      name: '選択した行の詳細',
    })
    function readDetailOutput() {
      return {
        title: detail.querySelector('h3')?.textContent,
        href: screen
          .getByRole('link', { name: '銘柄ページを開く' })
          .getAttribute('href'),
      }
    }

    expect(readDetailOutput()).toEqual({
      title: '架空銘柄 C',
      href: '/stocks/DEMO-C',
    })
  })

  it('分類行を選ぶと分類ページへリンクする', async () => {
    const user = userEvent.setup()
    await renderWidget({
      as_of: '2099-01-01',
      rows: [
        {
          key: 'group:demo-axis/demo-group-a',
          name: '架空分類 A',
          values: { size: 7, change: 0.015 },
        },
      ],
    })
    await user.click(screen.getByText(/全件一覧/))
    await user.click(
      screen.getByRole('button', {
        name: '架空分類 A, change: +1.50%, 詳細を表示',
      }),
    )

    expect(
      screen
        .getByRole('link', { name: '分類ページを開く' })
        .getAttribute('href'),
    ).toBe('/groups/demo-axis/demo-group-a')
  })

  it('空・ゼロ・欠損の項目は空状態と全件一覧に残す', async () => {
    const user = userEvent.setup()
    await renderWidget({
      as_of: '2099-01-01',
      rows: [
        { key: 'stock:DEMO-A', name: '架空銘柄 A', values: { change: 0.012 } },
        {
          key: 'stock:DEMO-B',
          name: '架空銘柄 B',
          values: { size: 0, change: null },
        },
        {
          key: 'stock:DEMO-C',
          name: '架空銘柄 C',
          values: { size: 0, change: 0 },
        },
      ],
    })
    await user.click(screen.getByText(/全件一覧/))

    function readRowsAndStatus() {
      return {
        status: screen.getByRole('status').textContent,
        rows: screen
          .getAllByRole('button', { name: /詳細を表示/ })
          .map((row) => row.getAttribute('aria-label')),
      }
    }

    expect(readRowsAndStatus()).toEqual({
      status: '表示できるデータがありません。',
      rows: [
        '架空銘柄 A, change: +1.20%, 詳細を表示',
        '架空銘柄 B, change: —, 詳細を表示',
        '架空銘柄 C, change: 0.00%, 詳細を表示',
      ],
    })
  })

  it('行がない場合に空状態を表示する', async () => {
    await renderWidget({ as_of: '2099-01-01', rows: [] })

    function readEmptyState() {
      return {
        status: screen.getByRole('status').textContent,
        list: screen.queryByText(/全件一覧/),
      }
    }

    expect(readEmptyState()).toEqual({
      status: '表示できるデータがありません。',
      list: null,
    })
  })
})
