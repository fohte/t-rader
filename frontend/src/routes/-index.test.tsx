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
type NoteKind = components['schemas']['NoteKind']
type NoteRequest = {
  kind: string | null
  tag: string | null
  view: string | null
}

const noteKinds: NoteKind[] = [
  {
    key: 'sample-kind',
    display_name: '架空の種別表示名',
    requires_approval: false,
    sort_order: 0,
  },
]

const allNote: Note = {
  id: 'fake-note-a',
  version_id: 'fake-version-a',
  version_no: 1,
  is_current: true,
  title: '架空の確認メモ',
  body_md: '架空の本文',
  frontmatter_json: {},
  resolved_price_references_json: {},
  graphs_json: [],
  tags: ['架空タグ'],
  kind: 'sample-kind',
  status: 'approved',
  trigger: null,
  trigger_label: null,
  created_by_kind: 'human',
  created_at: '2026-01-12T02:00:00Z',
  updated_at: '2026-01-12T02:00:00Z',
}

const allNoteOverview: NoteOverview = {
  representative: allNote,
  current: {
    id: allNote.version_id,
    note_id: allNote.id,
    version_no: allNote.version_no,
    is_current: true,
    title: allNote.title,
    body_md: allNote.body_md,
    frontmatter_json: allNote.frontmatter_json,
    resolved_price_references_json: allNote.resolved_price_references_json,
    graphs_json: allNote.graphs_json,
    status: allNote.status,
    created_by_kind: allNote.created_by_kind,
    created_at: allNote.created_at,
  } satisfies NoteVersion,
  latest_unread: null,
  unread_version_count: 0,
}

function getCombinedFilterOutput(
  search: unknown,
  kindSelect: HTMLElement,
  noteRequests: NoteRequest[],
) {
  return {
    search,
    selectedKind: kindSelect.textContent,
    selectedTag: screen.getByRole('combobox', { name: 'タグで絞り込み' })
      .textContent,
    kindOptions: screen
      .getAllByRole('option')
      .map((option) => option.textContent),
    combinedQueries: noteRequests
      .filter(({ kind, tag }) => kind != null || tag != null)
      .map(({ kind, tag, view }) => ({ kind, tag, view })),
  }
}

function getUnknownKindFilterOutput(search: unknown, kindSelect: HTMLElement) {
  return {
    selectedKind: kindSelect.textContent,
    rawKeyVisible: screen.queryByText('unlisted-kind-key') != null,
    allKindsOption: screen.getByRole('option', { name: 'すべての種別' })
      .textContent,
    search,
  }
}

afterEach(() => {
  cleanup()
  resetFetch()
})

async function renderNotes(initialEntry: string) {
  const noteRequests: NoteRequest[] = []
  stubFetch((input, init) => {
    const request = input instanceof Request ? input : new Request(input, init)
    const url = new URL(request.url)

    if (url.pathname === '/api/notes/overview') {
      const kind = url.searchParams.get('kind')
      const tag = url.searchParams.get('tag')
      const view = url.searchParams.get('view')
      noteRequests.push({ kind, tag, view })
      const matches =
        (kind == null || allNote.kind === kind) &&
        (tag == null || allNote.tags.includes(tag))
      return Promise.resolve(Response.json(matches ? [allNoteOverview] : []))
    }
    if (url.pathname === '/api/note-kinds') {
      return Promise.resolve(Response.json(noteKinds))
    }
    if (url.pathname === '/api/annotations') {
      return Promise.resolve(Response.json([]))
    }
    return Promise.resolve(Response.json({}, { status: 404 }))
  })

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
  await screen.findByRole('combobox', { name: '種別で絞り込み' })
  return { router, noteRequests }
}

describe('NotesPage', () => {
  it('loads both filters from the URL and sends them together to the API', async () => {
    const user = userEvent.setup()
    const { router, noteRequests } = await renderNotes(
      '/notes?kind=sample-kind&tag=%E6%9E%B6%E7%A9%BA%E3%82%BF%E3%82%B0',
    )
    await screen.findByRole('heading', { level: 1, name: /> ノート/ })
    const kindSelect = screen.getByRole('combobox', {
      name: '種別で絞り込み',
    })
    await user.click(kindSelect)
    await screen.findByRole('option', { name: 'すべての種別' })

    expect(
      getCombinedFilterOutput(
        router.state.location.search,
        kindSelect,
        noteRequests,
      ),
    ).toEqual({
      search: { kind: 'sample-kind', tag: '架空タグ' },
      selectedKind: '架空の種別表示名',
      selectedTag: '架空タグ',
      kindOptions: ['すべての種別', '架空の種別表示名'],
      combinedQueries: [{ kind: 'sample-kind', tag: '架空タグ', view: 'all' }],
    })
  }, 30_000)

  it('keeps the other filter when changing or clearing one filter', async () => {
    const user = userEvent.setup()
    const { router } = await renderNotes(
      '/notes?tag=%E6%9E%B6%E7%A9%BA%E3%82%BF%E3%82%B0',
    )

    await user.click(screen.getByRole('combobox', { name: '種別で絞り込み' }))
    await user.click(
      await screen.findByRole('option', { name: '架空の種別表示名' }),
    )
    await waitFor(() => {
      expect(router.state.location.search).toEqual({
        kind: 'sample-kind',
        tag: '架空タグ',
      })
    })

    await user.click(screen.getByRole('combobox', { name: '種別で絞り込み' }))
    await user.click(
      await screen.findByRole('option', { name: 'すべての種別' }),
    )
    await waitFor(() => {
      expect(router.state.location.search).toEqual({ tag: '架空タグ' })
    })
  }, 30_000)

  it('clears both filters with the clear action', async () => {
    const user = userEvent.setup()
    const { router } = await renderNotes(
      '/notes?kind=sample-kind&tag=%E6%9E%B6%E7%A9%BA%E3%82%BF%E3%82%B0',
    )

    await user.click(screen.getByRole('button', { name: '条件を解除' }))
    await waitFor(() => {
      expect(router.state.location.search).toEqual({})
    })
  }, 30_000)

  it('keeps the selected kind when choosing a tag', async () => {
    const user = userEvent.setup()
    const { router } = await renderNotes('/notes?kind=sample-kind')

    await user.click(screen.getByRole('combobox', { name: 'タグで絞り込み' }))
    await user.click(await screen.findByRole('option', { name: '架空タグ' }))
    await waitFor(() => {
      expect(router.state.location.search).toEqual({
        kind: 'sample-kind',
        tag: '架空タグ',
      })
    })
  }, 30_000)

  it('keeps both filters while switching tabs', async () => {
    const user = userEvent.setup()
    const { router } = await renderNotes(
      '/notes?kind=sample-kind&tag=%E6%9E%B6%E7%A9%BA%E3%82%BF%E3%82%B0',
    )

    await user.click(screen.getByRole('tab', { name: 'アノテーション' }))
    await waitFor(() => {
      expect(router.state.location.search).toEqual({
        kind: 'sample-kind',
        tag: '架空タグ',
        tab: 'annotations',
      })
    })

    await user.click(screen.getByRole('tab', { name: 'ノート' }))
    await waitFor(() => {
      expect(router.state.location.search).toEqual({
        kind: 'sample-kind',
        tag: '架空タグ',
      })
    })
  }, 30_000)

  it('keeps an unknown URL kind selected until the user clears it', async () => {
    const user = userEvent.setup()
    const { router } = await renderNotes('/notes?kind=unlisted-kind-key')

    await screen.findByText('未登録の種別')
    const kindSelect = screen.getByRole('combobox', {
      name: '種別で絞り込み',
    })
    await user.click(kindSelect)
    await screen.findByRole('option', { name: 'すべての種別' })

    expect(
      getUnknownKindFilterOutput(router.state.location.search, kindSelect),
    ).toEqual({
      selectedKind: '未登録の種別',
      rawKeyVisible: false,
      allKindsOption: 'すべての種別',
      search: { kind: 'unlisted-kind-key' },
    })

    await user.click(screen.getByRole('option', { name: 'すべての種別' }))
    await waitFor(() => {
      expect(router.state.location.search).toEqual({})
    })
  }, 30_000)
})
