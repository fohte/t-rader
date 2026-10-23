import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import {
  createMemoryHistory,
  createRouter,
  RouterProvider,
} from '@tanstack/react-router'
import { cleanup, render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it } from 'vitest'

import type { components } from '#lib/api/schema.gen'
import { resetFetch, stubFetch } from '#test/stub-fetch.test-helper'

type Note = components['schemas']['Note']
type NoteVersion = components['schemas']['NoteVersion']
type NoteOverview = components['schemas']['NoteOverviewResponse']

const representative: Note = {
  id: 'fake-note-a',
  version_id: 'fake-version-a',
  version_no: 2,
  is_current: true,
  title: '架空の確認ノート',
  body_md: '架空の本文です。',
  frontmatter_json: {},
  resolved_price_references_json: {},
  graphs_json: [],
  tags: ['sample-tag'],
  kind: 'sample-kind',
  status: 'approved',
  trigger: null,
  trigger_label: null,
  created_by_kind: 'human',
  created_at: '2026-01-01T00:00:00Z',
  updated_at: '2026-01-01T00:00:00Z',
}

const unreadVersion: NoteVersion = {
  id: 'fake-version-unread',
  note_id: representative.id,
  version_no: 3,
  is_current: false,
  title: representative.title,
  body_md: '架空の更新本文です。',
  frontmatter_json: {},
  resolved_price_references_json: {},
  graphs_json: [],
  status: 'unread',
  created_by_kind: 'llm',
  created_at: '2026-01-02T00:00:00Z',
}

const overview: NoteOverview = {
  representative,
  current: {
    ...unreadVersion,
    id: representative.version_id,
    version_no: representative.version_no,
    is_current: true,
    status: 'approved',
    created_by_kind: representative.created_by_kind,
    created_at: representative.created_at,
  },
  latest_unread: unreadVersion,
  unread_version_count: 1,
}

afterEach(() => {
  cleanup()
  resetFetch()
})

describe('/notes overview route', () => {
  it('keeps the view and both note filters in the URL and overview request', async () => {
    const overviewRequests: Record<string, string | undefined>[] = []
    stubFetch((input, init) => {
      const request =
        input instanceof Request ? input : new Request(input, init)
      const url = new URL(request.url)

      if (url.pathname === '/api/notes/overview') {
        overviewRequests.push({
          kind: url.searchParams.get('kind') ?? undefined,
          tag: url.searchParams.get('tag') ?? undefined,
          view: url.searchParams.get('view') ?? undefined,
        })
        return Promise.resolve(Response.json([overview]))
      }
      if (url.pathname === '/api/note-kinds') {
        return Promise.resolve(
          Response.json([
            {
              key: 'sample-kind',
              display_name: '架空の種別表示名',
              requires_approval: true,
              description: null,
              sort_order: 0,
            },
          ]),
        )
      }
      if (url.pathname === '/api/annotations') {
        return Promise.resolve(Response.json([]))
      }
      return Promise.resolve(Response.json([]))
    })

    const { routeTree } = await import('#routeTree.gen')
    const router = createRouter({
      routeTree,
      history: createMemoryHistory({
        initialEntries: ['/notes?view=pending&kind=sample-kind&tag=sample-tag'],
      }),
    })
    const queryClient = new QueryClient({
      defaultOptions: { queries: { retry: false } },
    })
    render(<RouterProvider router={router} />, {
      wrapper: ({ children }) => (
        <QueryClientProvider client={queryClient}>
          {children}
        </QueryClientProvider>
      ),
    })

    await screen.findByRole('link', { name: '架空の確認ノート' })
    await userEvent.click(screen.getByRole('button', { name: 'すべて' }))
    await waitFor(() => {
      expect(router.state.location.search).toEqual({
        view: 'all',
        kind: 'sample-kind',
        tag: 'sample-tag',
      })
    })

    const normalizedRequests = [
      ...new Set(overviewRequests.map((request) => JSON.stringify(request))),
    ].sort()
    expect(normalizedRequests).toEqual(
      [
        JSON.stringify({ kind: 'sample-kind', tag: 'sample-tag', view: 'all' }),
        JSON.stringify({
          kind: 'sample-kind',
          tag: 'sample-tag',
          view: 'pending',
        }),
        JSON.stringify({ view: 'all' }),
      ].sort(),
    )
  }, 30_000)
})
