import { Link, RouterProvider } from '@tanstack/react-router'
import { cleanup, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { useState } from 'react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import {
  type ReviewQueueData,
  type ReviewQueueItem,
  reviewQueueModel,
} from '#components/review/review-queue-model'
import { useReviewNavigation } from '#components/review/use-review-navigation'
import { useReviewQueue } from '#components/review/use-review-queue'
import { createStoryRouter } from '#storybook/story-router'

vi.mock('#components/review/use-review-queue', () => ({
  useReviewQueue: vi.fn(),
}))

type ReviewQueueState = ReturnType<typeof useReviewQueue>

const currentItemKey = 'annotation:sample-annotation-01'
const item: ReviewQueueItem = {
  kind: 'annotation',
  id: 'sample-annotation-01',
  title: 'サンプルの注釈',
  createdAt: '2026-01-14T03:00:00Z',
  targetSymbol: 'sample-code',
  timestamp: '2026-01-14T02:30:00Z',
  price: null,
}
const reviewNoteItem: ReviewQueueItem = {
  kind: 'note',
  id: 'fake-review-version',
  noteId: 'fake-review-note',
  versionNo: 2,
  title: '架空のレビュー対象ノート',
  createdAt: '2026-01-14T04:00:00Z',
  noteKind: null,
  createdByKind: 'llm',
  unreadVersionCount: 1,
}
const mixedItems = [reviewNoteItem, item]
const mixedSnapshot = reviewQueueModel.createNavigationSnapshot(mixedItems)
const mixedReviewNavigationState = {
  snapshot: mixedSnapshot,
  currentItemKey: 'note:fake-review-version',
  items: mixedItems,
}
const queueData: ReviewQueueData = {
  counts: { notes: 0, annotations: 1, total: 1 },
  items: [item],
}
const mixedQueueData: ReviewQueueData = {
  counts: { notes: 1, annotations: 1, total: 2 },
  items: mixedItems,
}
const invalidate = vi.fn()
const pendingQueue: ReviewQueueState = {
  data: null,
  isPending: true,
  isError: false,
  invalidate,
}
const loadedQueue: ReviewQueueState = {
  data: queueData,
  isPending: false,
  isError: false,
  invalidate,
}
const scopedItems: ReviewQueueItem[] = [
  {
    kind: 'note',
    id: 'fake-version-approved',
    noteId: 'fake-note-approved',
    versionNo: 2,
    title: '架空の確認済みノート',
    createdAt: '2026-01-14T02:00:00Z',
    noteKind: null,
    createdByKind: 'human',
    unreadVersionCount: 0,
  },
  {
    kind: 'note',
    id: 'fake-version-next',
    noteId: 'fake-note-next',
    versionNo: 3,
    title: '架空の次のノート',
    createdAt: '2026-01-14T01:00:00Z',
    noteKind: null,
    createdByKind: 'llm',
    unreadVersionCount: 1,
  },
]
const scopedSnapshot = reviewQueueModel.createNavigationSnapshot(scopedItems)
const scopedNavigationState = {
  snapshot: scopedSnapshot,
  currentItemKey: 'note:fake-version-approved',
  items: scopedItems,
  scope: 'notes' as const,
}

afterEach(cleanup)

function ReviewNavigationProbe() {
  const [queue, setQueue] = useState(pendingQueue)
  vi.mocked(useReviewQueue).mockReturnValue(queue)
  const { navigation, isPending, isError } = useReviewNavigation(currentItemKey)

  return (
    <>
      <output data-testid="review-navigation-state">
        {JSON.stringify({ navigation, isPending, isError })}
      </output>
      <button
        type="button"
        onClick={() => {
          setQueue(loadedQueue)
        }}
      >
        Load queue
      </button>
    </>
  )
}

function ScopedReviewNavigationProbe() {
  const { navigation, isPending, isError } = useReviewNavigation(
    'note:fake-version-approved',
    'fake-note-approved',
  )

  return (
    <>
      <Link
        to="/notes/$noteId"
        params={{ noteId: 'fake-note-approved' }}
        state={(previous) => ({
          ...previous,
          reviewQueue: scopedNavigationState,
        })}
      >
        Open scoped notes navigation
      </Link>
      <output data-testid="review-navigation-state">
        {JSON.stringify({ navigation, isPending, isError })}
      </output>
    </>
  )
}

function LegacyReviewNavigationProbe() {
  const { navigation, isPending, isError } = useReviewNavigation(
    'note:fake-review-version',
    'fake-review-note',
  )

  return (
    <>
      <Link
        to="/notes/$noteId"
        params={{ noteId: 'fake-review-note' }}
        state={(previous) => ({
          ...previous,
          reviewQueue: mixedReviewNavigationState,
        })}
      >
        Open note from review
      </Link>
      <output data-testid="review-navigation-state">
        {JSON.stringify({ navigation, isPending, isError })}
      </output>
    </>
  )
}

describe('useReviewNavigation', () => {
  it('creates a fallback snapshot after a direct-open queue finishes loading', async () => {
    const user = userEvent.setup()
    const router = createStoryRouter(() => <ReviewNavigationProbe />, {
      paths: ['/annotations/$annoId'],
      initialPath: '/annotations/sample-annotation-01',
    })
    render(<RouterProvider router={router} />)

    const output = await screen.findByTestId('review-navigation-state')
    expect(output.textContent).toEqual(
      JSON.stringify({ navigation: null, isPending: true, isError: false }),
    )

    await user.click(screen.getByRole('button', { name: 'Load queue' }))

    const expectedNavigation = {
      items: [item],
      snapshot: reviewQueueModel.createNavigationSnapshot([item]),
      currentItemKey,
    }
    const expected = JSON.stringify({
      navigation: expectedNavigation,
      isPending: false,
      isError: false,
    })

    expect((await screen.findByText(expected)).textContent).toEqual(expected)
  })

  it('keeps approved overview rows when the live unread queue excludes them', async () => {
    const user = userEvent.setup()
    vi.mocked(useReviewQueue).mockReturnValue({
      data: { counts: { notes: 0, annotations: 0, total: 0 }, items: [] },
      isPending: false,
      isError: false,
      invalidate,
    })
    const router = createStoryRouter(() => <ScopedReviewNavigationProbe />, {
      paths: ['/notes/$noteId'],
      initialPath: '/notes/fake-note-approved',
    })
    render(<RouterProvider router={router} />)

    await user.click(
      await screen.findByRole('link', {
        name: 'Open scoped notes navigation',
      }),
    )

    const expected = JSON.stringify({
      navigation: {
        items: scopedItems,
        snapshot: scopedSnapshot,
        currentItemKey: 'note:fake-version-approved',
        scope: 'notes',
      },
      isPending: false,
      isError: false,
    })
    expect(
      (await screen.findByTestId('review-navigation-state')).textContent,
    ).toEqual(expected)
  })

  it('keeps the mixed queue snapshot when opening a note from the unified list', async () => {
    const user = userEvent.setup()
    vi.mocked(useReviewQueue).mockReturnValue({
      data: mixedQueueData,
      isPending: false,
      isError: false,
      invalidate,
    })
    const router = createStoryRouter(() => <LegacyReviewNavigationProbe />, {
      paths: ['/notes', '/notes/$noteId'],
      initialPath: '/notes?view=pending',
    })
    render(<RouterProvider router={router} />)

    await user.click(
      await screen.findByRole('link', { name: 'Open note from review' }),
    )

    const expected = JSON.stringify({
      navigation: {
        items: mixedItems,
        snapshot: mixedSnapshot,
        currentItemKey: 'note:fake-review-version',
      },
      isPending: false,
      isError: false,
    })
    expect(
      (await screen.findByTestId('review-navigation-state')).textContent,
    ).toEqual(expected)
  })
})
