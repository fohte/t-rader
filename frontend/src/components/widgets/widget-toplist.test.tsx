import { RouterProvider } from '@tanstack/react-router'
import { act, cleanup, render, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import type { ECElementEvent } from 'echarts/core'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { WidgetToplist } from '#components/widgets'
import type { components } from '#lib/api/schema.gen'
import type { EChartsOptionFactory } from '#lib/echarts/use-echarts-chart'
import type { WidgetQueryDefinitions } from '#lib/widget-value-format'
import { createStoryRouter } from '#storybook/story-router'

type ChartClickHandler = (event: ECElementEvent) => void

const chartHarness = vi.hoisted(() => ({
  clickHandler: null as ChartClickHandler | null,
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
        updateOption: () => undefined,
        dispatchAction: () => undefined,
      }
    },
  }
})

type WidgetQueryResponse = components['schemas']['WidgetQueryResponse']
type WidgetToplistDefinition = Extract<
  components['schemas']['WidgetVisualizationRequest'],
  { type: 'toplist' }
>

const viz: WidgetToplistDefinition = {
  type: 'toplist',
  value: 'change',
  order: 'desc',
  limit: 3,
}
const queries: WidgetQueryDefinitions = {
  change: { metric: 'change', by: 'stock' },
}
const result: WidgetQueryResponse = {
  as_of: '2099-01-01',
  rows: [
    { key: 'stock:DEMO-C', name: '架空銘柄 C', values: { change: -0.006 } },
    { key: 'stock:DEMO-A', name: '架空銘柄 A', values: { change: 0.021 } },
    { key: 'stock:DEMO-D', name: '架空銘柄 D', values: { change: -0.014 } },
    { key: 'stock:DEMO-B', name: '架空銘柄 B', values: { change: 0.008 } },
  ],
}
const entityResult: WidgetQueryResponse = {
  as_of: '2099-04-12',
  rows: [
    {
      key: 'stock:DEMO-A',
      name: '架空銘柄 A',
      values: { change: 0.03 },
    },
    {
      key: 'indicator:DEMO-INDEX',
      name: '架空指標',
      values: { change: 0.02 },
    },
    {
      key: 'group:demo-axis/demo-group',
      name: '架空分類',
      values: { change: -0.01 },
    },
  ],
}

function resetChartHarness() {
  chartHarness.clickHandler = null
  chartHarness.options = []
}

async function renderWidget(
  widgetResult: WidgetQueryResponse = result,
  widgetViz: WidgetToplistDefinition = viz,
  props: {
    display?: components['schemas']['WidgetQueryRequest']['display']
    initialSelectedRowKey?: string
  } = {},
) {
  const router = createStoryRouter(
    () => (
      <WidgetToplist
        {...props}
        queries={queries}
        result={widgetResult}
        viz={widgetViz}
      />
    ),
    {
      paths: [
        '/groups/$axisKey/$groupKey',
        '/indicators/$indicatorId',
        '/stocks/$stockId',
      ],
    },
  )
  const rendered = render(<RouterProvider router={router} />)
  const readyRole = widgetResult.rows.length === 0 ? 'status' : 'img'
  await screen.findByRole(readyRole)
  return rendered
}

function readRankedRows() {
  return screen.getAllByRole('listitem').map((item) => {
    const button = item.querySelector('button')
    return {
      label: button?.getAttribute('aria-label') ?? null,
      value: button?.lastElementChild?.textContent ?? null,
    }
  })
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null
}

function isLabelFormatter(
  value: unknown,
): value is (params: { dataIndex: number }) => unknown {
  return typeof value === 'function'
}

function readChartOutput() {
  const option = chartHarness.options.at(-1)
  if (!isRecord(option)) return null
  const xAxisValue = Array.isArray(option.xAxis)
    ? option.xAxis[0]
    : option.xAxis
  const yAxisValue = Array.isArray(option.yAxis)
    ? option.yAxis[0]
    : option.yAxis
  const seriesValue = Array.isArray(option.series)
    ? option.series[0]
    : option.series
  if (
    !isRecord(xAxisValue) ||
    !isRecord(yAxisValue) ||
    !isRecord(seriesValue)
  ) {
    return null
  }

  const rows = Array.isArray(seriesValue.data) ? seriesValue.data : []
  const markLine = isRecord(seriesValue.markLine) ? seriesValue.markLine : {}
  const seriesLabel = isRecord(seriesValue.label) ? seriesValue.label : {}
  const labelFormatter = seriesLabel.formatter
  return {
    axis: {
      minimum: xAxisValue.min,
      maximum: xAxisValue.max,
      scale: xAxisValue.scale,
    },
    clip: seriesValue.clip,
    categories: yAxisValue.data,
    bars: rows.map((row, index) => {
      const data = isRecord(row) ? row : {}
      const itemStyle = isRecord(data.itemStyle) ? data.itemStyle : {}
      const label = isRecord(data.label) ? data.label : {}
      return {
        value: data.value,
        color: itemStyle.color,
        labelPosition: label.position,
        labelColor: label.color ?? seriesLabel.color,
        formattedLabel: isLabelFormatter(labelFormatter)
          ? (labelFormatter({ dataIndex: index }) ?? null)
          : null,
      }
    }),
    zeroReference: markLine.data,
  }
}

function readConfiguredScaleOutput() {
  return {
    header: screen.getByText('指定尺度: 0 / 0 / 100').textContent,
    chart: readChartOutput(),
  }
}

function readChartTransitionOutput(
  initialContainer: Element | null,
  initiallyHidden: boolean | null,
  populatedContainer: Element | null,
) {
  return {
    retainedNode: initialContainer === populatedContainer,
    initiallyHidden,
    populatedHidden:
      populatedContainer instanceof HTMLElement
        ? populatedContainer.hasAttribute('hidden')
        : null,
    categories: readChartOutput()?.categories,
  }
}

function readSelectedDetailOutput(detail: HTMLElement) {
  return {
    heading: within(detail).getByRole('heading', { level: 3 }).textContent,
    fields: Array.from(detail.querySelectorAll('dd'), (element) =>
      element.textContent.trim(),
    ),
    href: within(detail)
      .getByRole('link', { name: '指標ページを開く' })
      .getAttribute('href'),
  }
}

function readSelectedStockOutput() {
  return {
    heading: screen.getByRole('heading', { level: 3 }).textContent,
    link: screen
      .getByRole('link', { name: '銘柄ページを開く' })
      .getAttribute('href'),
  }
}

function emitChartClick(dataIndex: number) {
  act(() => {
    chartHarness.clickHandler?.({
      type: 'click',
      componentType: 'series',
      componentSubType: 'bar',
      componentIndex: 0,
      seriesType: 'bar',
      name: '',
      dataIndex,
      data: {},
      value: 0,
      $vars: [],
    })
  })
}

afterEach(() => {
  cleanup()
  resetChartHarness()
})

describe('WidgetToplist', () => {
  it('降順と limit を適用し、順位と符号付きの表示値を読み上げられる', async () => {
    const user = userEvent.setup()
    await renderWidget(result, { ...viz, limit: 2 })
    await user.click(screen.getByText('順位一覧から詳細を選ぶ (2)'))

    expect(readRankedRows()).toEqual([
      {
        label: '1 位: 架空銘柄 A、DEMO-A、+2.10%、詳細を表示',
        value: '+2.10%',
      },
      {
        label: '2 位: 架空銘柄 B、DEMO-B、+0.80%、詳細を表示',
        value: '+0.80%',
      },
    ])
  })

  it('昇順では小さい値から表示し、同値をコード順に並べる', async () => {
    const user = userEvent.setup()
    await renderWidget(
      {
        as_of: '2099-01-01',
        rows: [
          { key: 'stock:DEMO-C', name: '架空銘柄 C', values: { change: 0.01 } },
          {
            key: 'stock:DEMO-A',
            name: '架空銘柄 A',
            values: { change: -0.01 },
          },
          {
            key: 'stock:DEMO-B',
            name: '架空銘柄 B',
            values: { change: -0.01 },
          },
        ],
      },
      { ...viz, order: 'asc', limit: 3 },
    )
    await user.click(screen.getByText('順位一覧から詳細を選ぶ (3)'))

    expect(readRankedRows()).toEqual([
      {
        label: '1 位: 架空銘柄 A、DEMO-A、-1.00%、詳細を表示',
        value: '-1.00%',
      },
      {
        label: '2 位: 架空銘柄 B、DEMO-B、-1.00%、詳細を表示',
        value: '-1.00%',
      },
      {
        label: '3 位: 架空銘柄 C、DEMO-C、+1.00%、詳細を表示',
        value: '+1.00%',
      },
    ])
  })

  it('欠損値を除外し、式の表示形式と数値の符号を保つ', async () => {
    const user = userEvent.setup()
    await renderWidget(
      {
        as_of: '2099-01-01',
        rows: [
          {
            key: 'stock:DEMO-A',
            name: '架空銘柄 A',
            values: { relative: 0.0123 },
          },
          {
            key: 'stock:DEMO-B',
            name: '架空銘柄 B',
            values: { relative: null },
          },
          {
            key: 'stock:DEMO-C',
            name: '架空銘柄 C',
            values: { estimate: 1250.5 },
          },
          {
            key: 'stock:DEMO-D',
            name: '架空銘柄 D',
            values: {},
          },
        ],
      },
      { type: 'toplist', value: 'relative', order: 'desc', limit: 4 },
      {
        display: {
          relative: { label: '架空相対値', format: 'percent' },
          estimate: { label: '架空評価', format: 'number' },
        },
      },
    )
    await user.click(screen.getByText('順位一覧から詳細を選ぶ (1)'))

    expect(readRankedRows()).toEqual([
      {
        label: '1 位: 架空銘柄 A、DEMO-A、+1.23%、詳細を表示',
        value: '+1.23%',
      },
    ])
  })

  it('0を含まない指定尺度も0起点へ広げて正値を描く', async () => {
    await renderWidget(
      {
        as_of: '2099-04-12',
        rows: [
          {
            key: 'stock:DEMO-A',
            name: '架空銘柄 A',
            values: { estimate: 35 },
          },
        ],
      },
      {
        type: 'toplist',
        value: 'estimate',
        order: 'desc',
        limit: 1,
        bar_scale: { min: 10, max: 100 },
      },
      {
        display: {
          estimate: { label: '架空評価', format: 'number' },
        },
      },
    )

    expect(readConfiguredScaleOutput()).toEqual({
      header: '指定尺度: 0 / 0 / 100',
      chart: {
        axis: { minimum: 0, maximum: 100, scale: false },
        clip: true,
        categories: ['stock:DEMO-A'],
        bars: [
          {
            value: 35,
            color: '#ef5350',
            labelPosition: 'right',
            labelColor: '#0a0a0a',
            formattedLabel: '+35',
          },
        ],
        zeroReference: [{ xAxis: 0 }],
      },
    })
  })

  it('尺度を超える棒を端で止め、値のラベルは実際の値を表示する', async () => {
    await renderWidget(
      {
        as_of: '2099-01-01',
        rows: [
          {
            key: 'stock:DEMO-POSITIVE',
            name: '架空銘柄 正の外れ値',
            values: { change: 0.18 },
          },
          {
            key: 'stock:DEMO-INSIDE',
            name: '架空銘柄 尺度内',
            values: { change: 0.03 },
          },
          {
            key: 'stock:DEMO-NEGATIVE',
            name: '架空銘柄 負の外れ値',
            values: { change: -0.12 },
          },
        ],
      },
      { ...viz, limit: 3, bar_scale: { min: -0.04, max: 0.04 } },
    )

    expect(readChartOutput()).toEqual({
      axis: { minimum: -0.04, maximum: 0.04, scale: false },
      clip: true,
      categories: [
        'stock:DEMO-POSITIVE',
        'stock:DEMO-INSIDE',
        'stock:DEMO-NEGATIVE',
      ],
      bars: [
        {
          value: 0.04,
          color: '#ef5350',
          labelPosition: 'insideRight',
          labelColor: '#fafafa',
          formattedLabel: '+18.00%',
        },
        {
          value: 0.03,
          color: '#ef5350',
          labelPosition: 'right',
          labelColor: '#0a0a0a',
          formattedLabel: '+3.00%',
        },
        {
          value: -0.04,
          color: '#3f9fe0',
          labelPosition: 'insideLeft',
          labelColor: '#fafafa',
          formattedLabel: '-12.00%',
        },
      ],
      zeroReference: [{ xAxis: 0 }],
    })
  })

  it('chart のクリックで詳細を開く', async () => {
    await renderWidget(entityResult)
    emitChartClick(1)

    await screen.findByRole('heading', { level: 3 })
    const detail = screen.getByRole('region', { name: '選択した行の詳細' })
    expect(readSelectedDetailOutput(detail)).toEqual({
      heading: '架空指標',
      fields: ['2', 'DEMO-INDEX', '+2.00%', '2099-04-12'],
      href: '/indicators/DEMO-INDEX',
    })
  })

  it('キーボードで順位を選び詳細とリンクへ進める', async () => {
    const user = userEvent.setup()
    await renderWidget(entityResult)

    await user.tab()
    await user.keyboard('{Enter}')
    await user.tab()
    await user.keyboard('{Enter}')

    expect(readSelectedStockOutput()).toEqual({
      heading: '架空銘柄 A',
      link: '/stocks/DEMO-A',
    })
  })

  it('stock、indicator、group のそれぞれのアプリ内リンクへ遷移する', async () => {
    const user = userEvent.setup()
    await renderWidget(entityResult)
    await user.click(screen.getByText('順位一覧から詳細を選ぶ (3)'))

    const links = []
    for (const target of [
      {
        button: '1 位: 架空銘柄 A、DEMO-A、+3.00%、詳細を表示',
        name: '銘柄ページを開く',
      },
      {
        button: '2 位: 架空指標、DEMO-INDEX、+2.00%、詳細を表示',
        name: '指標ページを開く',
      },
      {
        button: '3 位: 架空分類、demo-axis/demo-group、-1.00%、詳細を表示',
        name: '分類ページを開く',
      },
    ]) {
      await user.click(screen.getByRole('button', { name: target.button }))
      links.push({
        heading: screen.getByRole('heading', { level: 3 }).textContent,
        href: screen
          .getByRole('link', { name: target.name })
          .getAttribute('href'),
      })
    }

    expect(links).toEqual([
      { heading: '架空銘柄 A', href: '/stocks/DEMO-A' },
      { heading: '架空指標', href: '/indicators/DEMO-INDEX' },
      { heading: '架空分類', href: '/groups/demo-axis/demo-group' },
    ])
  })

  it('値が null または欠落した行を表示しない', async () => {
    const user = userEvent.setup()
    await renderWidget({
      as_of: '2099-01-01',
      rows: [
        { key: 'stock:DEMO-A', name: '架空銘柄 A', values: { change: null } },
        { key: 'stock:DEMO-B', name: '架空銘柄 B', values: {} },
        {
          key: 'stock:DEMO-C',
          name: '架空銘柄 C',
          values: { change: 0.005 },
        },
      ],
    })
    await user.click(screen.getByText('順位一覧から詳細を選ぶ (1)'))

    expect(readRankedRows()).toEqual([
      {
        label: '1 位: 架空銘柄 C、DEMO-C、+0.50%、詳細を表示',
        value: '+0.50%',
      },
    ])
  })

  it('値を持つ行がない場合に空状態を表示する', async () => {
    await renderWidget({ as_of: '2099-01-01', rows: [] })

    expect(screen.getByRole('status').textContent).toBe(
      '表示できるデータがありません。',
    )
  })

  it('空状態でも chart container を維持し、データ取得後に表示する', () => {
    const emptyResult: WidgetQueryResponse = {
      as_of: '2099-01-01',
      rows: [],
    }
    const rendered = render(
      <WidgetToplist queries={queries} result={emptyResult} viz={viz} />,
    )
    const initialContainer = rendered.container.querySelector('[role="img"]')
    const initiallyHidden = initialContainer?.hasAttribute('hidden') ?? null

    rendered.rerender(
      <WidgetToplist queries={queries} result={result} viz={viz} />,
    )
    const populatedContainer = rendered.container.querySelector('[role="img"]')

    expect(
      readChartTransitionOutput(
        initialContainer,
        initiallyHidden,
        populatedContainer,
      ),
    ).toEqual({
      retainedNode: true,
      initiallyHidden: true,
      populatedHidden: false,
      categories: ['stock:DEMO-A', 'stock:DEMO-B', 'stock:DEMO-C'],
    })
  })
})
