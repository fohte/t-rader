import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { act, cleanup, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { NoteVersionReviewPanel } from '#components/note-detail/note-version-review-panel'
import type { NoteVersionReviewResult } from '#components/note-detail/note-version-review-view'
import type { NoteVersion } from '#lib/api/note-version-types'

const apiMock = vi.hoisted(() => {
  const callbacks: Record<
    string,
    { onSuccess?: () => void; onError?: (error: unknown) => void } | undefined
  > = {}

  return { callbacks }
})

vi.mock('#lib/api/client', () => ({
  $api: {
    useQuery: () => ({
      data: [{ parent_id: null, start_line: 1 }],
      isPending: false,
    }),
    useMutation: (_method: string, path: string) => ({
      isPending: false,
      isError: false,
      mutate: (
        _options: unknown,
        callbacks?: {
          onSuccess?: () => void
          onError?: (error: unknown) => void
        },
      ) => {
        apiMock.callbacks[path] = callbacks
      },
    }),
    queryOptions: (method: string, path: string, options?: unknown) => ({
      queryKey: [method, path, options],
    }),
  },
}))

const version: NoteVersion = {
  id: 'fake-version-a',
  note_id: 'fake-note-a',
  version_no: 2,
  title: '架空データの確認メモ',
  body_md: '変更された本文です。',
  frontmatter_json: {},
  resolved_price_references_json: {},
  graphs_json: [],
  status: 'unread',
  is_current: false,
  change_reason: null,
  created_by_kind: 'llm',
  execution_id: null,
  created_at: '2026-01-12T02:00:00Z',
  reviewed_at: null,
}

afterEach(() => {
  cleanup()
  apiMock.callbacks = {}
})

function renderPanel(onReviewed: (status: NoteVersionReviewResult) => void) {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
  })

  render(
    <QueryClientProvider client={queryClient}>
      <NoteVersionReviewPanel version={version} onReviewed={onReviewed} />
    </QueryClientProvider>,
  )

  return queryClient
}

describe('NoteVersionReviewPanel', () => {
  it.each([
    ['approve', '✓ 承認', 'approved'],
    ['reject', '✕ 却下', 'rejected'],
  ] as const)(
    'reports the %s result after its mutation succeeds',
    async (action, buttonName, expectedStatus) => {
      const user = userEvent.setup()
      const onReviewed = vi.fn<(status: NoteVersionReviewResult) => void>()
      renderPanel(onReviewed)

      await user.click(screen.getByRole('button', { name: buttonName }))
      const path = `/api/notes/{id}/versions/{n}/${action}`
      act(() => {
        apiMock.callbacks[path]?.onSuccess?.()
      })

      expect(onReviewed.mock.calls).toEqual([[expectedStatus]])
    },
  )

  it('does not report a review result before a mutation succeeds', async () => {
    const user = userEvent.setup()
    const onReviewed = vi.fn<(status: NoteVersionReviewResult) => void>()
    renderPanel(onReviewed)

    await user.click(screen.getByRole('button', { name: '✓ 承認' }))

    expect(onReviewed.mock.calls).toEqual([])
  })

  it('invalidates the note query for the reviewed version', async () => {
    const user = userEvent.setup()
    const onReviewed = vi.fn<(status: NoteVersionReviewResult) => void>()
    const queryClient = renderPanel(onReviewed)
    const invalidateQueries = vi.spyOn(queryClient, 'invalidateQueries')

    await user.click(screen.getByRole('button', { name: '✓ 承認' }))
    act(() => {
      apiMock.callbacks['/api/notes/{id}/versions/{n}/approve']?.onSuccess?.()
    })

    expect(
      invalidateQueries.mock.calls.map(([filters]) => filters?.queryKey),
    ).toEqual([
      [
        'get',
        '/api/notes/{id}/versions',
        { params: { path: { id: 'fake-note-a' } } },
      ],
      [
        'get',
        '/api/notes/{id}/versions/{n}',
        { params: { path: { id: 'fake-note-a', n: 2 } } },
      ],
      ['get', '/api/note-versions/pending', undefined],
      [
        'get',
        '/api/notes/{id}',
        {
          params: {
            path: { id: 'fake-note-a' },
            query: { version_id: 'fake-version-a' },
          },
        },
      ],
      ['get', '/api/notes', undefined],
      [
        'get',
        '/api/history',
        {
          params: {
            query: {
              target_kind: 'note',
              target_id: 'fake-note-a',
              limit: 50,
            },
          },
        },
      ],
    ])
  })
})
