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
})
