import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import {
  createMemoryHistory,
  createRouter,
  RouterProvider,
} from '@tanstack/react-router'
import { cleanup, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { pendingNoteOverview } from '#components/dashboard/dashboard-notes.fixtures'
import type { components } from '#lib/api/schema.gen'
import { resetFetch, stubFetch } from '#test/stub-fetch.test-helper'

vi.mock('#components/stock-page/stock-page-chart', () => ({
  StockPageChart: () => null,
}))

const stock: components['schemas']['Stock'] = {
  id: 'demo-code',
  name: '架空商事',
  market: null,
  sector_id: null,
  product_category: null,
  created_at: '2030-01-01T00:00:00Z',
  updated_at: '2030-01-01T00:00:00Z',
}

const group: components['schemas']['Group'] = {
  key: 'demo-group',
  name: '見本グループ',
  description: null,
  axis: { key: 'demo-axis', name: '見本分類' },
  stocks: [],
}

const detailRoutes = [
  {
    label: 'stock',
    path: '/stocks/demo-code?stockTab=notes',
    ref: 'stock:demo-code',
  },
  {
    label: 'indicator',
    path: '/indicators/demo-indicator',
    ref: 'indicator:demo-indicator',
  },
  {
    label: 'group',
    path: '/groups/demo-axis/demo-group',
    ref: 'group:demo-axis/demo-group',
  },
] as const

afterEach(() => {
  cleanup()
  resetFetch()
})

async function renderDetailRoute(path: string) {
  const noteRequests: { ref: string | null; view: string | null }[] = []
  stubFetch((input, init) => {
    const request = input instanceof Request ? input : new Request(input, init)
    const url = new URL(request.url)

    if (url.pathname === '/api/notes/overview') {
      noteRequests.push({
        ref: url.searchParams.get('ref'),
        view: url.searchParams.get('view'),
      })
      return Promise.resolve(Response.json([pendingNoteOverview]))
    }
    if (url.pathname === '/api/refs/stocks/demo-code') {
      return Promise.resolve(Response.json(stock))
    }
    if (url.pathname === '/api/refs/indicators/demo-indicator') {
      return Promise.resolve(
        Response.json({
          id: 'demo-indicator',
          kind: 'demo-kind',
          name: '見本指標',
        }),
      )
    }
    if (url.pathname === '/api/refs/indicators/demo-indicator/observations') {
      return Promise.resolve(Response.json([]))
    }
    if (url.pathname === '/api/refs/groups/demo-axis/demo-group') {
      return Promise.resolve(Response.json(group))
    }
    if (url.pathname === '/api/trades/summary') {
      return Promise.resolve(Response.json({ positions: [], realized_pnl: 0 }))
    }
    if (url.pathname.startsWith('/api/stocks/')) {
      return Promise.resolve(Response.json(null))
    }
    return Promise.resolve(Response.json([]))
  })

  const { routeTree } = await import('#routeTree.gen')
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  })
  const router = createRouter({
    routeTree,
    history: createMemoryHistory({ initialEntries: [path] }),
  })
  await router.load()
  render(
    <QueryClientProvider client={queryClient}>
      <RouterProvider router={router} />
    </QueryClientProvider>,
  )

  const pendingNoteLink = await screen.findByRole('link', {
    name: /現行なしの架空ノート/,
  })

  return {
    noteRequests,
    noteHref: pendingNoteLink.getAttribute('href'),
  }
}

describe('reference detail note routes', () => {
  it.each(detailRoutes)(
    '$label route loads notes using its reference',
    async ({ path, ref }) => {
      const output = await renderDetailRoute(path)

      expect(output).toEqual({
        noteRequests: [{ ref, view: 'all' }],
        noteHref: `/notes/${pendingNoteOverview.representative.id}`,
      })
    },
    30_000,
  )
})
