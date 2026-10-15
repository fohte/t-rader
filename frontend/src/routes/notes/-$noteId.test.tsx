import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import {
  createMemoryHistory,
  createRouter,
  RouterProvider,
} from '@tanstack/react-router'
import { cleanup, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import type { components } from '#lib/api/schema.gen'

type Note = components['schemas']['Note']
type NoteVersion = components['schemas']['NoteVersion']
type NoteKind = components['schemas']['NoteKind']
type FetchHandler = (
  input: RequestInfo | URL,
  init?: RequestInit,
) => Promise<Response>

const noteId = 'fake-note'
const versionId = 'fake-note-version'
const noteKindKey = 'fabricated-kind-key'
const noteKindLabel = '架空の種別表示名'

const pendingVersion: NoteVersion = {
  id: versionId,
  note_id: noteId,
  version_no: 1,
  is_current: false,
  title: '架空の分析ノート',
  body_md: '架空の本文です。',
  frontmatter_json: {},
  resolved_price_references_json: {},
  graphs_json: [],
  status: 'unread',
  created_by_kind: 'llm',
  created_at: '2099-01-01T00:00:00Z',
}

const note: Note = {
  id: noteId,
  version_id: versionId,
  version_no: 1,
  is_current: false,
  title: pendingVersion.title,
  body_md: pendingVersion.body_md,
  frontmatter_json: {},
  resolved_price_references_json: {},
  graphs_json: [],
  tags: [],
  kind: noteKindKey,
  status: 'unread',
  created_by_kind: 'llm',
  created_at: pendingVersion.created_at,
  updated_at: pendingVersion.created_at,
  execution_id: null,
}

const noteKind: NoteKind = {
  key: noteKindKey,
  display_name: noteKindLabel,
  requires_approval: true,
  sort_order: 0,
}

let fetchHandler: FetchHandler | null = null

function activeFetch(
  input: RequestInfo | URL,
  init?: RequestInit,
): Promise<Response> {
  return (
    fetchHandler?.(input, init) ??
    Promise.resolve(new Response(null, { status: 500 }))
  )
}

function stubFetch(handler: FetchHandler): void {
  fetchHandler = handler
  vi.stubGlobal('fetch', activeFetch)
}

function readNoteDetailOutput(
  noteRequests: Array<{
    method: string
    path: string
    versionId: string | null
  }>,
  unexpectedRequests: string[],
) {
  return {
    noteRequests,
    unexpectedRequests,
    title: screen.getByRole('heading', { level: 1 }).textContent,
    noteKindLabel: screen.getByText(noteKindLabel).textContent,
  }
}

afterEach(() => {
  cleanup()
  vi.unstubAllGlobals()
  fetchHandler = null
})

describe('NoteDetailPage', () => {
  it('shows the kind label for a pending note without a current version', async () => {
    const noteRequests: Array<{
      method: string
      path: string
      versionId: string | null
    }> = []
    const unexpectedRequests: string[] = []

    stubFetch((input: RequestInfo | URL, init?: RequestInit) => {
      const request =
        input instanceof Request ? input : new Request(input, init)
      const url = new URL(request.url)

      if (url.pathname === `/api/notes/${noteId}/versions`) {
        return Promise.resolve(Response.json([pendingVersion]))
      }
      if (url.pathname === `/api/notes/${noteId}`) {
        noteRequests.push({
          method: request.method,
          path: url.pathname,
          versionId: url.searchParams.get('version_id'),
        })
        if (url.searchParams.get('version_id') !== versionId) {
          return Promise.resolve(Response.json({}, { status: 404 }))
        }
        return Promise.resolve(Response.json(note))
      }
      if (url.pathname === '/api/note-kinds') {
        return Promise.resolve(Response.json([noteKind]))
      }
      if (url.pathname === `/api/notes/${noteId}/links`) {
        return Promise.resolve(Response.json({ incoming: [], outgoing: [] }))
      }
      if (url.pathname === '/api/history') {
        return Promise.resolve(Response.json([]))
      }
      if (
        url.pathname === '/api/strategies' ||
        url.pathname === '/api/agent-configs'
      ) {
        return Promise.resolve(Response.json([]))
      }
      if (
        url.pathname === `/api/notes/${noteId}/trades` ||
        url.pathname === `/api/notes/${noteId}/predictions` ||
        url.pathname === `/api/comments` ||
        url.pathname === '/api/note-versions/pending' ||
        url.pathname === '/api/annotations'
      ) {
        return Promise.resolve(Response.json([]))
      }
      unexpectedRequests.push(`${request.method} ${url.pathname}`)
      return Promise.resolve(Response.json({}, { status: 404 }))
    })

    const { routeTree } = await import('#routeTree.gen')
    const router = createRouter({
      routeTree,
      history: createMemoryHistory({ initialEntries: [`/notes/${noteId}`] }),
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

    await screen.findByText(noteKindLabel)

    expect(readNoteDetailOutput(noteRequests, unexpectedRequests)).toEqual({
      noteRequests: [
        {
          method: 'GET',
          path: `/api/notes/${noteId}`,
          versionId,
        },
      ],
      unexpectedRequests: [],
      title: '架空の分析ノート',
      noteKindLabel,
    })
  })
})
