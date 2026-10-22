import { cleanup, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { WidgetLine } from '#components/widgets/widget-line'
import type { components } from '#lib/api/schema.gen'
import type { WidgetQueryDefinitions } from '#lib/widget-value-format'

const chartHarness = vi.hoisted(() => ({ error: null as string | null }))

vi.mock('#lib/echarts/use-echarts-chart', async () => {
  const React = await import('react')
  return {
    useEChartsChart: () => ({
      containerRef: React.useRef<HTMLDivElement>(null),
      error: chartHarness.error,
    }),
  }
})

type WidgetQueryResponse = components['schemas']['WidgetQueryResponse']
type WidgetLineDefinition = Extract<
  components['schemas']['WidgetVisualizationRequest'],
  { type: 'line' }
>

const queries: WidgetQueryDefinitions = {
  movement: { metric: 'change', period: '20d', by: 'group' },
}
const display = { movement: { label: '騰落率' } }
const viz: WidgetLineDefinition = { type: 'line', series: ['movement'] }
const result: WidgetQueryResponse = {
  as_of: '2099-01-07',
  rows: [],
  series: [
    {
      key: 'movement:group:demo-axis/demo-group-a',
      query: 'movement',
      name: '騰落率',
      target: {
        key: 'group:demo-axis/demo-group-a',
        name: 'サンプル分類 A',
      },
      points: [
        { date: '2099-01-05', value: 0.01 },
        { date: '2099-01-06', value: null },
        { date: '2099-01-07', value: 0.03 },
      ],
    },
  ],
}

function readWidgetLineOutput() {
  return {
    chart: screen.queryByRole('img')?.getAttribute('aria-label') ?? null,
    period: screen.queryByText('2099-01-05 – 2099-01-07')?.textContent ?? null,
    status: screen.queryByRole('status')?.textContent ?? null,
    alert: screen.queryByRole('alert')?.textContent ?? null,
  }
}

afterEach(() => {
  cleanup()
  chartHarness.error = null
})

describe('WidgetLine', () => {
  it('期間、系列、凡例名を含む折れ線グラフを表示する', () => {
    render(
      <WidgetLine
        display={display}
        queries={queries}
        result={result}
        viz={viz}
      />,
    )

    expect(readWidgetLineOutput()).toEqual({
      chart:
        '2099-01-05 から 2099-01-07の折れ線グラフ。系列: 騰落率 · サンプル分類 A。欠損値は線をつなぎません。',
      period: '2099-01-05 – 2099-01-07',
      status: null,
      alert: null,
    })
  })

  it('値がない系列は空の状態を表示する', () => {
    render(
      <WidgetLine
        display={display}
        queries={queries}
        result={{
          as_of: '2099-01-07',
          rows: [],
          series: [
            {
              key: 'movement:group:demo-axis/demo-group-a',
              query: 'movement',
              name: '騰落率',
              points: [
                { date: '2099-01-05', value: null },
                { date: '2099-01-06', value: null },
              ],
            },
          ],
        }}
        viz={viz}
      />,
    )

    expect(readWidgetLineOutput()).toEqual({
      chart: null,
      period: null,
      status: '表示できる値がありません。',
      alert: null,
    })
  })

  it('描画に失敗した理由を表示する', () => {
    chartHarness.error = '架空の描画エラー'
    render(
      <WidgetLine
        display={display}
        queries={queries}
        result={result}
        viz={viz}
      />,
    )

    expect(readWidgetLineOutput()).toEqual({
      chart:
        '2099-01-05 から 2099-01-07の折れ線グラフ。系列: 騰落率 · サンプル分類 A。欠損値は線をつなぎません。',
      period: '2099-01-05 – 2099-01-07',
      status: null,
      alert: '架空の描画エラー',
    })
  })
})
