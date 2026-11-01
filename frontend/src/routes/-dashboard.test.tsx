import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import {
  createMemoryHistory,
  createRouter,
  RouterProvider,
} from '@tanstack/react-router'
import { cleanup, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it } from 'vitest'

import { pendingNoteOverview } from '#components/dashboard/dashboard-notes.fixtures'
import { resetFetch, stubFetch } from '#test/stub-fetch.test-helper'

afterEach(() => {
  cleanup()
  resetFetch()
})

function getPendingNoteOutput(
  requestedViews: { view: string | null }[],
  noteLink: HTMLElement,
) {
  return {
    requestedViews,
    href: noteLink.getAttribute('href'),
    status: screen.getByText('未レビュー').textContent,
    current: screen.getByText('現行なし').textContent,
  }
}

describe('DashboardRoute', () => {
  it('loads overview notes and displays a pending note without a current version', async () => {
    const noteRequests: { view: string | null }[] = []
    stubFetch((input, init) => {
      const request =
        input instanceof Request ? input : new Request(input, init)
      const url = new URL(request.url)

      if (url.pathname === '/api/notes/overview') {
        const view = url.searchParams.get('view')
        noteRequests.push({ view })
        return Promise.resolve(
          Response.json(view === 'all' ? [pendingNoteOverview] : []),
        )
      }
      if (url.pathname === '/api/trades/summary') {
        return Promise.resolve(
          Response.json({ positions: [], realized_pnl: 0 }),
        )
      }
      if (url.pathname === '/api/calendar/events') {
        return Promise.resolve(Response.json({ events: [] }))
      }
      return Promise.resolve(Response.json([]))
    })

    const { routeTree } = await import('#routeTree.gen')
    const queryClient = new QueryClient({
      defaultOptions: { queries: { retry: false } },
    })
    const router = createRouter({
      routeTree,
      history: createMemoryHistory({ initialEntries: ['/'] }),
    })
    await router.load()
    render(
      <QueryClientProvider client={queryClient}>
        <RouterProvider router={router} />
      </QueryClientProvider>,
    )

    await screen.findByRole('heading', { level: 1, name: 'ホーム' })
    const noteLink = await screen.findByRole('link', {
      name: /現行なしの架空ノート/,
    })

    expect(getPendingNoteOutput(noteRequests, noteLink)).toEqual({
      requestedViews: [{ view: 'all' }],
      href: `/notes/${pendingNoteOverview.representative.id}`,
      status: '未レビュー',
      current: '現行なし',
    })
  }, 30_000)
})
