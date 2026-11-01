import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import {
  createMemoryHistory,
  createRouter,
  RouterProvider,
} from '@tanstack/react-router'
import { cleanup, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it } from 'vitest'

import type { components } from '#lib/api/schema.gen'
import { resetFetch, stubFetch } from '#test/stub-fetch.test-helper'

type Note = components['schemas']['Note']
type NoteVersion = components['schemas']['NoteVersion']
type NoteOverview = components['schemas']['NoteOverviewResponse']

const pendingNote: Note = {
  id: 'fake-note-pending',
  version_id: 'fake-version-pending',
  version_no: 1,
  is_current: false,
  title: '現行なしの架空ノート',
  body_md: '表示用の架空ノートです。',
  frontmatter_json: {},
  resolved_price_references_json: {},
  graphs_json: [],
  tags: [],
  kind: null,
  status: 'unread',
  trigger: null,
  trigger_label: null,
  created_by_kind: 'llm',
  created_at: '2026-10-10T03:00:00Z',
  updated_at: '2026-10-10T03:00:00Z',
}

const pendingVersion: NoteVersion = {
  id: pendingNote.version_id,
  note_id: pendingNote.id,
  version_no: pendingNote.version_no,
  is_current: false,
  title: pendingNote.title,
  body_md: pendingNote.body_md,
  frontmatter_json: pendingNote.frontmatter_json,
  resolved_price_references_json: pendingNote.resolved_price_references_json,
  graphs_json: pendingNote.graphs_json,
  warning_dispositions: [],
  status: pendingNote.status,
  created_by_kind: pendingNote.created_by_kind,
  created_at: pendingNote.created_at,
}

const pendingOverview: NoteOverview = {
  representative: pendingNote,
  current: null,
  latest_unread: pendingVersion,
  unread_version_count: 1,
}

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
          Response.json(view === 'all' ? [pendingOverview] : []),
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
      href: `/notes/${pendingNote.id}`,
      status: '未レビュー',
      current: '現行なし',
    })
  }, 30_000)
})
