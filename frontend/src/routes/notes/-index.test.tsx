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
  warning_dispositions: [],
  graphs_json: [],
  status: 'unread',
  created_by_kind: 'llm',
  created_at: '2026-01-02T00:00:00Z',
}

const warningUnreadVersion: NoteVersion = {
  ...unreadVersion,
  id: 'fake-version-warning',
  version_no: 4,
  body_md: '架空の値 42% を確認します。',
  warning_dispositions: [
    {
      warning_id: 'price:42%25:1',
      value: '42%',
      location: { kind: 'body', start: 13, end: 16 },
      reason: '架空の値の例外として示したため',
    },
  ],
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

const warningOverview: NoteOverview = {
  ...overview,
  latest_unread: warningUnreadVersion,
  unread_version_count: 2,
}

const taggedOverview: NoteOverview = {
  ...overview,
  representative: {
    ...representative,
    tags: ['theme-x', 'unlisted-tag', 'category-y'],
  },
}

function getTagCandidatesErrorOutput(alert: HTMLElement) {
  return { role: alert.getAttribute('role'), text: alert.textContent }
}

function getTagFilterRequestOutput(
  search: unknown,
  overviewRequests: Record<string, string | undefined>[],
) {
  return { search, overviewRequests }
}

async function renderNotesPage(initialEntry: string) {
  const { routeTree } = await import('#routeTree.gen')
  const router = createRouter({
    routeTree,
    history: createMemoryHistory({ initialEntries: [initialEntry] }),
  })
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  })
  render(<RouterProvider router={router} />, {
    wrapper: ({ children }) => (
      <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
    ),
  })
  return router
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
      if (url.pathname === '/api/note-tags') {
        return Promise.resolve(
          Response.json([{ key: 'sample-tag', display_name: '架空タグ' }]),
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

    await screen.findByRole(
      'link',
      { name: '架空の確認ノート' },
      { timeout: 15_000 },
    )
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
      ].sort(),
    )
  }, 30_000)

  it('shows registered tag labels in API order', async () => {
    const user = userEvent.setup()
    stubFetch((input, init) => {
      const request =
        input instanceof Request ? input : new Request(input, init)
      const url = new URL(request.url)

      if (url.pathname === '/api/notes/overview') {
        return Promise.resolve(Response.json([taggedOverview]))
      }
      if (url.pathname === '/api/note-tags') {
        return Promise.resolve(
          Response.json([
            { key: 'category-y', display_name: '分類Y' },
            { key: 'theme-x', display_name: 'テーマX' },
          ]),
        )
      }
      if (url.pathname === '/api/note-kinds') {
        return Promise.resolve(Response.json([]))
      }
      if (url.pathname === '/api/annotations') {
        return Promise.resolve(Response.json([]))
      }
      return Promise.resolve(Response.json([]))
    })

    await renderNotesPage('/notes')

    await screen.findByRole('link', { name: '架空の確認ノート' })
    await user.click(screen.getByRole('combobox', { name: 'タグで絞り込み' }))
    await screen.findByRole('option', { name: '分類Y' })
    const options = screen
      .getAllByRole('option')
      .map((option) => option.textContent)

    expect(options).toEqual(['すべてのタグ', '分類Y', 'テーマX'])
  }, 30_000)

  it('uses the selected tag key in the overview request', async () => {
    const user = userEvent.setup()
    const overviewRequests: Record<string, string | undefined>[] = []
    stubFetch((input, init) => {
      const request =
        input instanceof Request ? input : new Request(input, init)
      const url = new URL(request.url)

      if (url.pathname === '/api/notes/overview') {
        overviewRequests.push({
          tag: url.searchParams.get('tag') ?? undefined,
          view: url.searchParams.get('view') ?? undefined,
        })
        return Promise.resolve(Response.json([taggedOverview]))
      }
      if (url.pathname === '/api/note-tags') {
        return Promise.resolve(
          Response.json([
            { key: 'category-y', display_name: '分類Y' },
            { key: 'theme-x', display_name: 'テーマX' },
          ]),
        )
      }
      if (url.pathname === '/api/note-kinds') {
        return Promise.resolve(Response.json([]))
      }
      if (url.pathname === '/api/annotations') {
        return Promise.resolve(Response.json([]))
      }
      return Promise.resolve(Response.json([]))
    })

    const router = await renderNotesPage('/notes')
    await screen.findByRole('link', { name: '架空の確認ノート' })
    await user.click(screen.getByRole('combobox', { name: 'タグで絞り込み' }))
    await user.click(await screen.findByRole('option', { name: 'テーマX' }))

    await waitFor(() => {
      expect(
        getTagFilterRequestOutput(
          router.state.location.search,
          overviewRequests,
        ),
      ).toEqual({
        search: { tag: 'theme-x' },
        overviewRequests: [
          { tag: undefined, view: 'all' },
          { tag: 'theme-x', view: 'all' },
        ],
      })
    })
  }, 30_000)

  it('shows when the tag vocabulary could not be loaded', async () => {
    stubFetch((input, init) => {
      const request =
        input instanceof Request ? input : new Request(input, init)
      const url = new URL(request.url)

      if (url.pathname === '/api/notes/overview') {
        return Promise.resolve(Response.json([overview]))
      }
      if (url.pathname === '/api/note-tags') {
        return Promise.resolve(Response.json({}, { status: 500 }))
      }
      if (url.pathname === '/api/note-kinds') {
        return Promise.resolve(Response.json([]))
      }
      if (url.pathname === '/api/annotations') {
        return Promise.resolve(Response.json([]))
      }
      return Promise.resolve(Response.json([]))
    })

    const { routeTree } = await import('#routeTree.gen')
    const router = createRouter({
      routeTree,
      history: createMemoryHistory({ initialEntries: ['/notes'] }),
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

    const alert = await screen.findByRole('alert')

    expect(getTagCandidatesErrorOutput(alert)).toEqual({
      role: 'alert',
      text: 'タグ候補を読み込めませんでした。',
    })
  }, 30_000)

  it('requires reviewing warning details and individually confirms a warning version', async () => {
    const user = userEvent.setup()
    const approvalRequests: { method: string; body: unknown }[] = []
    stubFetch(async (input, init) => {
      const request =
        input instanceof Request ? input : new Request(input, init)
      const url = new URL(request.url)

      if (url.pathname === '/api/notes/overview') {
        return Response.json([warningOverview])
      }
      if (url.pathname === '/api/note-kinds') return Response.json([])
      if (url.pathname === '/api/annotations') return Response.json([])
      if (url.pathname.endsWith('/approve')) {
        approvalRequests.push({
          method: request.method,
          body: await request.clone().json(),
        })
        return Response.json({
          ...warningUnreadVersion,
          status: 'approved',
          is_current: true,
        })
      }
      return Response.json([])
    })

    const { routeTree } = await import('#routeTree.gen')
    const router = createRouter({
      routeTree,
      history: createMemoryHistory({
        initialEntries: ['/notes?view=pending'],
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
    await user.click(
      screen.getByRole('checkbox', {
        name: '架空の確認ノート の最新未読バージョンを選択',
      }),
    )
    const bulkApprovalButtons = screen.getAllByRole('button', {
      name: '選択した 1 件を承認',
    })
    for (const bulkApprovalButton of bulkApprovalButtons) {
      await user.click(bulkApprovalButton)
    }
    const confirmation = screen.getByRole('checkbox', {
      name: '理由と対象箇所を確認し、数値の例外として承認します',
    })
    await user.click(confirmation)
    for (const bulkApprovalButton of screen.getAllByRole('button', {
      name: '選択した 1 件を承認',
    })) {
      await user.click(bulkApprovalButton)
    }
    await user.click(
      screen.getByRole('button', { name: 'このバージョンを個別に承認' }),
    )

    await waitFor(() => {
      expect(approvalRequests).toEqual([
        { method: 'POST', body: { warning_dispositions_confirmed: true } },
      ])
    })
  }, 30_000)
})
