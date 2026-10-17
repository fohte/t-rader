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
  asOf: string,
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
      viz: { type: 'toplist', value: queryName, order: 'desc', limit: 5 },
      time: { as_of: asOf },
    },
  }
}

const dashboardConfig: DashboardConfig = {
  title: '市場の動き',
  widgets: [
    makeWidget('値動き', 'movement', 'change', 'DEMO-A', '2099-01-05'),
    makeWidget('時価総額', 'capitalization', 'mkt_cap', 'DEMO-B', '2098-12-01'),
  ],
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

let activeMiddleware: ReturnType<typeof installWidgetQueryCapture> | null = null

afterEach(() => {
  cleanup()
  activeMiddleware?.eject()
  activeMiddleware = null
})

describe('useDashboardPageData', () => {
  it('初期表示では最新を使い、日付指定後に最新へ戻せる', async () => {
    activeMiddleware = installWidgetQueryCapture()
    const queryClient = new QueryClient({
      defaultOptions: { queries: { retry: false } },
    })
    function Wrapper({ children }: { children: ReactNode }) {
      return (
        <QueryClientProvider client={queryClient}>
          {children}
        </QueryClientProvider>
      )
    }
    const hook = renderHook(
      () => useDashboardPageData(dashboardConfig, 'sample-market'),
      { wrapper: Wrapper },
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
        viz: { type: 'toplist', value: 'movement', order: 'desc', limit: 5 },
      },
      {
        queries: {
          capitalization: {
            metric: 'mkt_cap',
            by: 'stock',
            universe: { stocks: ['DEMO-B'] },
          },
        },
        viz: {
          type: 'toplist',
          value: 'capitalization',
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
        viz: { type: 'toplist', value: 'movement', order: 'desc', limit: 5 },
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
        viz: {
          type: 'toplist',
          value: 'capitalization',
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
        viz: { type: 'toplist', value: 'movement', order: 'desc', limit: 5 },
      },
      {
        queries: {
          capitalization: {
            metric: 'mkt_cap',
            by: 'stock',
            universe: { stocks: ['DEMO-B'] },
          },
        },
        viz: {
          type: 'toplist',
          value: 'capitalization',
          order: 'desc',
          limit: 5,
        },
      },
    ])
  })
})
