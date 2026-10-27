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
  const requests: Record<string, unknown[]> = {}

  return { callbacks, requests }
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
        options: unknown,
        callbacks?: {
          onSuccess?: () => void
          onError?: (error: unknown) => void
        },
      ) => {
        apiMock.callbacks[path] = callbacks
        apiMock.requests[path] = [...(apiMock.requests[path] ?? []), options]
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
  warning_dispositions: [],
  graphs_json: [],
  status: 'unread',
  is_current: false,
  change_reason: null,
  created_by_kind: 'llm',
  execution_id: null,
  created_at: '2026-01-12T02:00:00Z',
  reviewed_at: null,
}

const warningVersion: NoteVersion = {
  ...version,
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

function getQueryInvalidationOutput(
  noteVersionsInvalidated: (boolean | undefined)[],
  overviewInvalidated: boolean | undefined,
) {
  return { noteVersionsInvalidated, overviewInvalidated }
}

function getWarningApprovalOutput(
  approve: HTMLElement,
  confirmation: HTMLElement,
) {
  return {
    approveDisabled: approve.matches(':disabled'),
    warningValue: screen.getByText(
      (_, element) =>
        element?.tagName === 'P' && element.textContent === '対象: 42%',
    ).textContent,
    location: screen.getByText('本文 byte 13–16').textContent,
    reason: screen.getByText('理由: 架空の値の例外として示したため')
      .textContent,
    confirmationChecked: confirmation.getAttribute('aria-checked'),
  }
}

function getChangedVersionOutput() {
  return {
    confirmationChecked: screen
      .getByRole('checkbox', {
        name: '理由と対象箇所を確認し、数値の例外として承認します',
      })
      .getAttribute('aria-checked'),
    approveDisabled: screen
      .getByRole('button', { name: '✓ 承認' })
      .matches(':disabled'),
  }
}

afterEach(() => {
  cleanup()
  apiMock.callbacks = {}
  apiMock.requests = {}
})

function renderPanel(
  onReviewed: (status: NoteVersionReviewResult) => void,
  reviewVersion: NoteVersion = version,
) {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
  })

  render(
    <QueryClientProvider client={queryClient}>
      <NoteVersionReviewPanel version={reviewVersion} onReviewed={onReviewed} />
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

  it('requires confirmation and sends it with the warning version approval', async () => {
    const user = userEvent.setup()
    const onReviewed = vi.fn<(status: NoteVersionReviewResult) => void>()
    renderPanel(onReviewed, warningVersion)
    const approve = screen.getByRole('button', { name: '✓ 承認' })
    const confirmation = screen.getByRole('checkbox', {
      name: '理由と対象箇所を確認し、数値の例外として承認します',
    })

    expect(getWarningApprovalOutput(approve, confirmation)).toEqual({
      approveDisabled: true,
      warningValue: '対象: 42%',
      location: '本文 byte 13–16',
      reason: '理由: 架空の値の例外として示したため',
      confirmationChecked: 'false',
    })

    await user.click(confirmation)
    await user.click(approve)

    expect(apiMock.requests['/api/notes/{id}/versions/{n}/approve']).toEqual([
      {
        params: {
          path: { id: warningVersion.note_id, n: warningVersion.version_no },
        },
        body: { warning_dispositions_confirmed: true },
      },
    ])
  })

  it('does not carry a warning confirmation to another version', async () => {
    const user = userEvent.setup()
    const queryClient = new QueryClient({
      defaultOptions: {
        queries: { retry: false },
        mutations: { retry: false },
      },
    })
    const { rerender } = render(
      <QueryClientProvider client={queryClient}>
        <NoteVersionReviewPanel
          version={warningVersion}
          onReviewed={() => {}}
        />
      </QueryClientProvider>,
    )
    await user.click(
      screen.getByRole('checkbox', {
        name: '理由と対象箇所を確認し、数値の例外として承認します',
      }),
    )
    rerender(
      <QueryClientProvider client={queryClient}>
        <NoteVersionReviewPanel
          version={{ ...warningVersion, id: 'fake-version-b', version_no: 3 }}
          onReviewed={() => {}}
        />
      </QueryClientProvider>,
    )

    expect(getChangedVersionOutput()).toEqual({
      confirmationChecked: 'false',
      approveDisabled: true,
    })
  })

  it('invalidates note queries for every version of the reviewed note', async () => {
    const user = userEvent.setup()
    const onReviewed = vi.fn<(status: NoteVersionReviewResult) => void>()
    const queryClient = renderPanel(onReviewed)
    const overviewQueryKey = ['get', '/api/notes/overview', undefined] as const
    queryClient.setQueryData(overviewQueryKey, [])
    const versionQueryKeys = ['fake-version-a', 'fake-version-b'].map(
      (versionId) =>
        [
          'get',
          '/api/notes/{id}',
          {
            params: {
              path: { id: version.note_id },
              query: { version_id: versionId },
            },
          },
        ] as const,
    )
    for (const queryKey of versionQueryKeys) {
      queryClient.setQueryData(queryKey, { kind: 'fabricated-kind-key' })
    }

    await user.click(screen.getByRole('button', { name: '✓ 承認' }))
    act(() => {
      apiMock.callbacks['/api/notes/{id}/versions/{n}/approve']?.onSuccess?.()
    })

    expect(
      getQueryInvalidationOutput(
        versionQueryKeys.map(
          (queryKey) => queryClient.getQueryState(queryKey)?.isInvalidated,
        ),
        queryClient.getQueryState(overviewQueryKey)?.isInvalidated,
      ),
    ).toEqual({
      noteVersionsInvalidated: [true, true],
      overviewInvalidated: true,
    })
  })
})
