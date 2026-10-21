import { RouterProvider } from '@tanstack/react-router'
import { cleanup, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it, vi } from 'vitest'

import {
  type DashboardPageState,
  DashboardPageView,
} from '#components/dashboard-pages/dashboard-page-view'
import { createStoryRouter } from '#storybook/story-router'

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

afterEach(cleanup)

function readTableOutput(table: HTMLElement) {
  return {
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

describe('DashboardPageView', () => {
  it('日付指定時に最新を選ぶと空文字で通知する', async () => {
    const user = userEvent.setup()
    const onAsOfChange = vi.fn<(date: string) => void>()
    const state: DashboardPageState = {
      status: 'ready',
      title: 'サンプル',
      asOf: '2099-02-03',
      onAsOfChange,
      widgets: [],
    }
    const router = createStoryRouter(
      () => <DashboardPageView state={state} />,
      { paths: ['/dashboards'] },
    )

    render(<RouterProvider router={router} />)
    await user.click(await screen.findByRole('button', { name: '最新' }))

    expect(onAsOfChange.mock.calls).toEqual([['']])
  })

  it('change query を符号付きパーセントで表示する', async () => {
    const state: DashboardPageState = {
      status: 'ready',
      title: '市場の動き',
      asOf: '2099-01-05',
      onAsOfChange: () => undefined,
      widgets: [
        {
          key: 'movement-list',
          title: '値動き',
          width: 'half',
          definition: {
            queries: {
              movement: { metric: 'change', by: 'stock' },
            },
            viz: {
              type: 'toplist',
              value: 'movement',
              order: 'desc',
              limit: 5,
            },
          },
          isPending: false,
          errorMessage: null,
          result: {
            as_of: '2099-01-05',
            rows: [
              {
                key: 'stock:DEMO-A',
                name: '架空銘柄 A',
                values: { movement: 0.0042 },
              },
            ],
          },
        },
      ],
    }
    render(<DashboardPageView state={state} showDashboardListLink={false} />)

    expect(await screen.findByRole('img')).toHaveAttribute(
      'aria-label',
      'change の toplist。基準日 2099-01-05。1 件を降順で表示。1 位 架空銘柄 A、DEMO-A、+0.42%',
    )
  })

  it('table widget を設定列順でダッシュボードに表示する', () => {
    const state: DashboardPageState = {
      status: 'ready',
      title: '市場の動き',
      asOf: '2099-01-05',
      onAsOfChange: () => undefined,
      widgets: [
        {
          key: 'valuation-table',
          title: '評価指標',
          width: 'full',
          definition: {
            queries: {
              close: { metric: 'close', by: 'stock' },
              pbr: { metric: 'pbr', by: 'stock' },
            },
            display: {
              close: { label: '終値', format: 'number' },
              pbr: { label: 'PBR', format: 'number' },
            },
            viz: { type: 'table', columns: ['pbr', 'close'] },
          },
          isPending: false,
          errorMessage: null,
          result: {
            as_of: '2099-01-05',
            rows: [
              {
                key: 'stock:DEMO-A',
                name: '架空銘柄 A',
                values: { close: 1250, pbr: 1.25 },
              },
              {
                key: 'stock:DEMO-B',
                name: '架空銘柄 B',
                values: { pbr: null },
              },
            ],
          },
        },
      ],
    }
    render(<DashboardPageView state={state} showDashboardListLink={false} />)
    const table = screen.getByRole('table', { name: 'ウィジェットの表' })

    expect(readTableOutput(table)).toEqual({
      headers: ['対象', 'PBR', '終値'],
      rows: [
        ['架空銘柄 A', '1.25', '1,250'],
        ['架空銘柄 B', '—', '—'],
      ],
    })
  })
})
