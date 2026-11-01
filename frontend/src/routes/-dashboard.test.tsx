import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import {
  createMemoryHistory,
  createRouter,
  RouterProvider,
} from '@tanstack/react-router'
import { cleanup, render, screen, within } from '@testing-library/react'
import { afterEach, describe, expect, it } from 'vitest'

import {
  currentAndUnreadNoteOverview,
  pendingNoteOverview,
} from '#components/dashboard/dashboard-notes.fixtures'
import { resetFetch, stubFetch } from '#test/stub-fetch.test-helper'

afterEach(() => {
  cleanup()
  resetFetch()
})

function getDashboardNotesOutput(
  requestedViews: { view: string | null }[],
  pendingNoteLink: HTMLElement,
  currentAndUnreadNoteLink: HTMLElement,
) {
  return {
    requestedViews,
    notes: [
      {
        href: pendingNoteLink.getAttribute('href'),
        status: within(pendingNoteLink).getByText('未レビュー').textContent,
        current: within(pendingNoteLink).getByText('現行なし').textContent,
        unreadVersions:
          within(pendingNoteLink).getByText('未読 1 バージョン').textContent,
      },
      {
        href: currentAndUnreadNoteLink.getAttribute('href'),
        status: within(currentAndUnreadNoteLink).getByText('承認済み')
          .textContent,
        current:
          within(currentAndUnreadNoteLink).queryByText('現行なし')
            ?.textContent ?? null,
        unreadVersions: within(currentAndUnreadNoteLink).getByText(
          '未読 1 バージョン',
        ).textContent,
      },
    ],
  }
}

describe('DashboardRoute', () => {
  it('loads pending notes with and without a current version', async () => {
    const noteRequests: { view: string | null }[] = []
    stubFetch((input, init) => {
      const request =
        input instanceof Request ? input : new Request(input, init)
      const url = new URL(request.url)

      if (url.pathname === '/api/notes/overview') {
        const view = url.searchParams.get('view')
        noteRequests.push({ view })
        return Promise.resolve(
          Response.json(
            view === 'all'
              ? [pendingNoteOverview, currentAndUnreadNoteOverview]
              : [],
          ),
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
    const pendingNoteLink = await screen.findByRole('link', {
      name: /現行なしの架空ノート/,
    })
    const currentAndUnreadNoteLink = await screen.findByRole('link', {
      name: /現行版と未読版を持つ架空ノート/,
    })

    expect(
      getDashboardNotesOutput(
        noteRequests,
        pendingNoteLink,
        currentAndUnreadNoteLink,
      ),
    ).toEqual({
      requestedViews: [{ view: 'all' }],
      notes: [
        {
          href: `/notes/${pendingNoteOverview.representative.id}`,
          status: '未レビュー',
          current: '現行なし',
          unreadVersions: '未読 1 バージョン',
        },
        {
          href: `/notes/${currentAndUnreadNoteOverview.representative.id}`,
          status: '承認済み',
          current: null,
          unreadVersions: '未読 1 バージョン',
        },
      ],
    })
  }, 30_000)
})
