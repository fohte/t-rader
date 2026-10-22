import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { act, cleanup, renderHook, waitFor } from '@testing-library/react'
import type { Middleware } from 'openapi-fetch'
import type { ReactNode } from 'react'
import { afterEach, describe, expect, it } from 'vitest'

import type { DashboardConfig } from '#components/dashboard-pages/dashboard-config'
import { useDashboardPageData } from '#components/dashboard-pages/use-dashboard-page-data'
import { fetchClient } from '#lib/api/client'

function makeWidget(
  title: string,
  queryName: string,
  metric: 'change' | 'mkt_cap',
  stockId: string,
  asOf?: string,
): DashboardConfig['widgets'][number] {
  return {
    title,
    width: 'half',
    definition: {
      queries: {
        [queryName]: {
          metric,
          ...(metric === 'change' ? { period: '1d' } : {}),
          by: 'stock',
          universe: { stocks: [stockId] },
        },
      },
      formulas: { doubled: `${queryName} * 2` },
      viz: { type: 'toplist', value: 'doubled', order: 'desc', limit: 5 },
      ...(asOf == null ? {} : { time: { as_of: asOf } }),
    },
  }
}

const dashboardConfig: DashboardConfig = {
  title: '市場の動き',
  widgets: [
    makeWidget('値動き', 'movement', 'change', 'DEMO-A'),
    makeWidget('時価総額', 'capitalization', 'mkt_cap', 'DEMO-B'),
  ],
}

const dashboardConfigWithFixedDate: DashboardConfig = {
  title: '市場の動き',
  widgets: [makeWidget('値動き', 'movement', 'change', 'DEMO-A', '2099-01-05')],
}

function createQueryClientWrapper() {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  })
  return function Wrapper({ children }: { children: ReactNode }) {
    return (
      <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
    )
  }
}

function installWidgetQueryCapture() {
  const requests: unknown[] = []
  const middleware: Middleware = {
    async onRequest({ request }) {
      const url = new URL(request.url)
      if (url.pathname !== '/api/widgets/query') return

      const body: unknown = await request.clone().json()
      requests.push(body)
      return new Response(JSON.stringify({ as_of: '2099-03-01', rows: [] }), {
        headers: { 'content-type': 'application/json' },
      })
    },
  }
  fetchClient.use(middleware)
  return {
    requests,
    eject: () => {
      fetchClient.eject(middleware)
    },
  }
}

function readTableQueryOutput(requests: unknown[], widgets: unknown[]) {
  return { requests, widgets }
}

let activeMiddleware: ReturnType<typeof installWidgetQueryCapture> | null = null

afterEach(() => {
  cleanup()
  activeMiddleware?.eject()
  activeMiddleware = null
})

describe('useDashboardPageData', () => {
  it('ウィジェットの表示設定を API と表示 state に渡す', async () => {
    activeMiddleware = installWidgetQueryCapture()
    const displayConfig: DashboardConfig = {
      title: '市場の動き',
      widgets: [
        {
          title: '値動き',
          width: 'half',
          definition: {
            queries: {
              movement: { metric: 'change', period: '1d', by: 'stock' },
            },
            display: { movement: { label: 'Movement', format: 'percent' } },
            viz: {
              type: 'toplist',
              value: 'movement',
              order: 'desc',
              limit: 5,
            },
          },
        },
      ],
    }
    const hook = renderHook(
      () => useDashboardPageData(displayConfig, 'sample-market'),
      { wrapper: createQueryClientWrapper() },
    )

    await waitFor(() => {
      if (activeMiddleware?.requests.length !== 1) {
        throw new Error('widget request is not complete')
      }
    })

    expect(
      activeMiddleware.requests.concat(
        hook.result.current.widgets.map((widget) => widget.definition.display),
      ),
    ).toEqual([
      {
        queries: {
          movement: { metric: 'change', period: '1d', by: 'stock' },
        },
        display: { movement: { label: 'Movement', format: 'percent' } },
        viz: { type: 'toplist', value: 'movement', order: 'desc', limit: 5 },
      },
      { movement: { label: 'Movement', format: 'percent' } },
    ])
  })

  it('table widget の列順と query 定義を API と表示 state に渡す', async () => {
    activeMiddleware = installWidgetQueryCapture()
    const tableConfig: DashboardConfig = {
      title: 'サンプル表',
      widgets: [
        {
          title: '値の比較',
          width: 'full',
          definition: {
            queries: {
              metric_a: { metric: 'close', by: 'stock' },
              metric_b: { metric: 'mkt_cap', by: 'stock' },
            },
            display: {
              metric_a: { label: '指標 A', format: 'number' },
              metric_b: { label: '指標 B', format: 'number' },
            },
            viz: {
              type: 'table',
              columns: ['metric_b', 'metric_a'],
            },
          },
        },
      ],
    }
    const hook = renderHook(
      () => useDashboardPageData(tableConfig, 'sample-market'),
      { wrapper: createQueryClientWrapper() },
    )

    await waitFor(() => {
      if (activeMiddleware?.requests.length !== 1) {
        throw new Error('widget request is not complete')
      }
    })

    const queries = {
      metric_a: { metric: 'close', by: 'stock' },
      metric_b: { metric: 'mkt_cap', by: 'stock' },
    }
    const display = {
      metric_a: { label: '指標 A', format: 'number' },
      metric_b: { label: '指標 B', format: 'number' },
    }
    const viz = {
      type: 'table',
      columns: ['metric_b', 'metric_a'],
    }
    expect(
      readTableQueryOutput(
        activeMiddleware.requests,
        hook.result.current.widgets.map((widget) => widget.definition),
      ),
    ).toEqual({
      requests: [{ queries, display, viz }],
      widgets: [{ queries, display, viz }],
    })
  })

  it('初期表示では最新を使い、日付指定後に最新へ戻せる', async () => {
    activeMiddleware = installWidgetQueryCapture()
    const hook = renderHook(
      () => useDashboardPageData(dashboardConfig, 'sample-market'),
      { wrapper: createQueryClientWrapper() },
    )

    await waitFor(() => {
      if (activeMiddleware?.requests.length !== 2) {
        throw new Error('widget requests are not complete')
      }
    })

    act(() => {
      hook.result.current.onAsOfChange('2099-03-01')
    })

    await waitFor(() => {
      if (activeMiddleware?.requests.length !== 4) {
        throw new Error('selected-date requests are not complete')
      }
    })

    act(() => {
      hook.result.current.onAsOfChange('')
    })

    await waitFor(() => {
      if (activeMiddleware?.requests.length !== 6) {
        throw new Error('latest-date requests are not complete')
      }
    })

    expect(activeMiddleware.requests).toEqual([
      {
        queries: {
          movement: {
            metric: 'change',
            period: '1d',
            by: 'stock',
            universe: { stocks: ['DEMO-A'] },
          },
        },
        formulas: { doubled: 'movement * 2' },
        viz: { type: 'toplist', value: 'doubled', order: 'desc', limit: 5 },
      },
      {
        queries: {
          capitalization: {
            metric: 'mkt_cap',
            by: 'stock',
            universe: { stocks: ['DEMO-B'] },
          },
        },
        formulas: { doubled: 'capitalization * 2' },
        viz: {
          type: 'toplist',
          value: 'doubled',
          order: 'desc',
          limit: 5,
        },
      },
      {
        queries: {
          movement: {
            metric: 'change',
            period: '1d',
            by: 'stock',
            universe: { stocks: ['DEMO-A'] },
          },
        },
        formulas: { doubled: 'movement * 2' },
        viz: { type: 'toplist', value: 'doubled', order: 'desc', limit: 5 },
        time: { as_of: '2099-03-01' },
      },
      {
        queries: {
          capitalization: {
            metric: 'mkt_cap',
            by: 'stock',
            universe: { stocks: ['DEMO-B'] },
          },
        },
        formulas: { doubled: 'capitalization * 2' },
        viz: {
          type: 'toplist',
          value: 'doubled',
          order: 'desc',
          limit: 5,
        },
        time: { as_of: '2099-03-01' },
      },
      {
        queries: {
          movement: {
            metric: 'change',
            period: '1d',
            by: 'stock',
            universe: { stocks: ['DEMO-A'] },
          },
        },
        formulas: { doubled: 'movement * 2' },
        viz: { type: 'toplist', value: 'doubled', order: 'desc', limit: 5 },
      },
      {
        queries: {
          capitalization: {
            metric: 'mkt_cap',
            by: 'stock',
            universe: { stocks: ['DEMO-B'] },
          },
        },
        formulas: { doubled: 'capitalization * 2' },
        viz: {
          type: 'toplist',
          value: 'doubled',
          order: 'desc',
          limit: 5,
        },
      },
    ])
  })

  it('ウィジェットの固定基準日を使い、画面で選んだ日付を優先する', async () => {
    activeMiddleware = installWidgetQueryCapture()
    const hook = renderHook(
      () => useDashboardPageData(dashboardConfigWithFixedDate, 'sample-market'),
      { wrapper: createQueryClientWrapper() },
    )

    await waitFor(() => {
      if (activeMiddleware?.requests.length !== 1) {
        throw new Error('widget request is not complete')
      }
    })

    act(() => {
      hook.result.current.onAsOfChange('2099-03-01')
    })

    await waitFor(() => {
      if (activeMiddleware?.requests.length !== 2) {
        throw new Error('selected-date request is not complete')
      }
    })

    act(() => {
      hook.result.current.onAsOfChange('')
    })

    await waitFor(() => {
      if (activeMiddleware?.requests.length !== 3) {
        throw new Error('latest-date request is not complete')
      }
    })

    expect(activeMiddleware.requests).toEqual([
      {
        queries: {
          movement: {
            metric: 'change',
            period: '1d',
            by: 'stock',
            universe: { stocks: ['DEMO-A'] },
          },
        },
        formulas: { doubled: 'movement * 2' },
        viz: { type: 'toplist', value: 'doubled', order: 'desc', limit: 5 },
        time: { as_of: '2099-01-05' },
      },
      {
        queries: {
          movement: {
            metric: 'change',
            period: '1d',
            by: 'stock',
            universe: { stocks: ['DEMO-A'] },
          },
        },
        formulas: { doubled: 'movement * 2' },
        viz: { type: 'toplist', value: 'doubled', order: 'desc', limit: 5 },
        time: { as_of: '2099-03-01' },
      },
      {
        queries: {
          movement: {
            metric: 'change',
            period: '1d',
            by: 'stock',
            universe: { stocks: ['DEMO-A'] },
          },
        },
        formulas: { doubled: 'movement * 2' },
        viz: { type: 'toplist', value: 'doubled', order: 'desc', limit: 5 },
      },
    ])
  })

  it('line widget の期間を保ち、画面で選んだ基準日を API に渡す', async () => {
    activeMiddleware = installWidgetQueryCapture()
    const lineConfig: DashboardConfig = {
      title: '期間推移',
      widgets: [
        {
          title: 'サンプル指標',
          width: 'full',
          definition: {
            queries: {
              sample: {
                metric: 'indicator',
                by: 'none',
                indicator: 'demo-indicator',
              },
            },
            viz: { type: 'line', series: ['sample'] },
            time: { range: '1y' },
          },
        },
      ],
    }
    const hook = renderHook(
      () => useDashboardPageData(lineConfig, 'sample-market'),
      { wrapper: createQueryClientWrapper() },
    )

    await waitFor(() => {
      if (activeMiddleware?.requests.length !== 1) {
        throw new Error('widget request is not complete')
      }
    })

    act(() => {
      hook.result.current.onAsOfChange('2099-03-01')
    })

    await waitFor(() => {
      if (activeMiddleware?.requests.length !== 2) {
        throw new Error('selected-date request is not complete')
      }
    })

    expect(activeMiddleware.requests).toEqual([
      {
        queries: {
          sample: {
            metric: 'indicator',
            by: 'none',
            indicator: 'demo-indicator',
          },
        },
        viz: { type: 'line', series: ['sample'] },
        time: { range: '1y' },
      },
      {
        queries: {
          sample: {
            metric: 'indicator',
            by: 'none',
            indicator: 'demo-indicator',
          },
        },
        viz: { type: 'line', series: ['sample'] },
        time: { range: '1y', as_of: '2099-03-01' },
      },
    ])
  })
})
