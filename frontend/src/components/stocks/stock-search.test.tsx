import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { Outlet, RouterProvider } from '@tanstack/react-router'
import { cleanup, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import type { Middleware } from 'openapi-fetch'
import type { ReactNode } from 'react'
import { afterEach, describe, expect, it } from 'vitest'

import { StockSearch } from '#components/stocks/stock-search'
import { fetchClient } from '#lib/api/client'
import type { components } from '#lib/api/schema.gen'
import { createStoryRouter } from '#storybook/story-router'

const sampleStock: components['schemas']['Stock'] = {
  id: 'demo-code',
  name: 'デモ商事',
  created_at: '2026-01-01T00:00:00Z',
  updated_at: '2026-01-01T00:00:00Z',
}

function installStockSearchMiddleware() {
  const queries: string[] = []
  const middleware: Middleware = {
    onRequest({ request }) {
      const url = new URL(request.url)
      if (url.pathname === '/api/refs/stocks') {
        queries.push(url.searchParams.get('q') ?? '')
        return new Response(JSON.stringify([sampleStock]), {
          status: 200,
          headers: { 'content-type': 'application/json' },
        })
      }
      throw new Error(`unmocked: ${request.method} ${url}`)
    },
  }
  fetchClient.use(middleware)
  return {
    queries,
    eject: () => {
      fetchClient.eject(middleware)
    },
  }
}

let active: ReturnType<typeof installStockSearchMiddleware> | null = null

function renderStockSearch() {
  active?.eject()
  active = installStockSearchMiddleware()

  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  })
  const router = createStoryRouter(
    () => (
      <>
        <StockSearch />
        <Outlet />
      </>
    ),
    {
      paths: [
        { path: '/portfolio', component: () => null },
        {
          path: '/stocks/$stockId',
          component: () => <p>銘柄ページ</p>,
        },
      ],
      initialPath: '/portfolio',
    },
  )
  function Wrapper({ children }: { children: ReactNode }) {
    return (
      <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
    )
  }
  render(<RouterProvider router={router} />, { wrapper: Wrapper })
  return router
}

function getStockSearchSnapshot(
  router: ReturnType<typeof createStoryRouter>,
  selectedHref: string | null,
) {
  return {
    searchQueries: active?.queries ?? [],
    pathname: router.state.location.pathname,
    selectedHref,
    searchInputVisible:
      screen.queryByRole('textbox', {
        name: '銘柄名またはコードで検索',
      }) != null,
  }
}

afterEach(() => {
  cleanup()
  active?.eject()
  active = null
})

describe('StockSearch', () => {
  it('閉じている間は検索 API を呼び出しません', () => {
    const router = renderStockSearch()

    expect(getStockSearchSnapshot(router, null)).toEqual({
      searchQueries: [],
      pathname: '/portfolio',
      selectedHref: null,
      searchInputVisible: false,
    })
  })

  it.each(['デモ商事', 'demo-code'])(
    '銘柄名またはコードの検索から銘柄ページへ遷移する (%s)',
    async (searchTerm) => {
      const user = userEvent.setup()
      const router = renderStockSearch()

      await user.click(
        await screen.findByRole('button', { name: '銘柄を探す' }),
      )
      const input = screen.getByRole('textbox', {
        name: '銘柄名またはコードで検索',
      })
      await user.click(input)
      await user.paste(searchTerm)
      const stockName = await screen.findByText('デモ商事')
      const selectedHref = stockName.closest('a')?.getAttribute('href') ?? null
      await user.click(stockName)
      await screen.findByText('銘柄ページ')

      expect(getStockSearchSnapshot(router, selectedHref)).toEqual({
        searchQueries: [searchTerm],
        pathname: '/stocks/demo-code',
        selectedHref: '/stocks/demo-code',
        searchInputVisible: false,
      })
    },
  )
})
