import { cleanup, renderHook } from '@testing-library/react'
import { afterEach, describe, expect, it } from 'vitest'

import type { ReviewQueueItem } from '#components/review/review-queue-model'
import { useReviewQueueNavigation } from '#components/review/use-review-queue-navigation'

const firstItem: ReviewQueueItem = {
  kind: 'note',
  id: 'fake-version-a',
  noteId: 'fake-note-a',
  versionNo: 2,
  title: '架空データの確認メモ',
  createdAt: '2026-01-12T02:00:00Z',
  noteKind: '架空の種別',
  createdByKind: 'llm',
  unreadVersionCount: 1,
}

const secondItem: ReviewQueueItem = {
  kind: 'annotation',
  id: 'fake-annotation-a',
  title: '架空データの注記',
  createdAt: '2026-01-12T01:00:00Z',
  targetSymbol: 'demo-code',
  timestamp: '2026-01-12T01:00:00Z',
  price: null,
}

afterEach(() => {
  cleanup()
})

describe('useReviewQueueNavigation', () => {
  it('reports loading and error states when queue items are unavailable', () => {
    const { result, rerender } = renderHook(
      ({ isError }) =>
        useReviewQueueNavigation({
          scopeKey: 'fake-note-a',
          currentItemKey: 'note:fake-version-a',
          items: undefined,
          isError,
        }),
      { initialProps: { isError: false } },
    )

    expect(result.current).toEqual({
      snapshot: null,
      state: { kind: 'loading' },
    })

    rerender({ isError: true })

    expect(result.current).toEqual({
      snapshot: null,
      state: { kind: 'error' },
    })
  })

  it('keeps the opened order while using the latest unread items', () => {
    const snapshot = {
      itemKeys: ['note:fake-version-a', 'annotation:fake-annotation-a'],
    }
    const { result } = renderHook(() =>
      useReviewQueueNavigation({
        scopeKey: 'fake-note-a',
        openedState: {
          snapshot,
          currentItemKey: 'note:fake-version-a',
        },
        currentItemKey: 'note:fake-version-a',
        items: [secondItem, firstItem],
        isError: false,
      }),
    )

    expect(result.current).toEqual({
      snapshot,
      state: {
        kind: 'item',
        total: 2,
        previousItem: null,
        nextItem: secondItem,
      },
    })
  })

  it('returns to review when the current item is the only unread item', () => {
    const { result } = renderHook(() =>
      useReviewQueueNavigation({
        scopeKey: 'fake-note-a',
        currentItemKey: 'note:fake-version-a',
        items: [firstItem],
        isError: false,
      }),
    )

    expect(result.current.state).toEqual({
      kind: 'only-current',
      total: 1,
    })
  })

  it('freezes the first loaded order for a direct note open', () => {
    const { result, rerender } = renderHook(
      ({ items }) =>
        useReviewQueueNavigation({
          scopeKey: 'fake-note-a',
          currentItemKey: 'note:fake-version-a',
          items,
          isError: false,
        }),
      { initialProps: { items: [firstItem, secondItem] } },
    )

    expect(result.current).toEqual({
      snapshot: {
        itemKeys: ['note:fake-version-a', 'annotation:fake-annotation-a'],
      },
      state: {
        kind: 'item',
        total: 2,
        previousItem: null,
        nextItem: secondItem,
      },
    })

    rerender({ items: [secondItem, firstItem] })

    expect(result.current).toEqual({
      snapshot: {
        itemKeys: ['note:fake-version-a', 'annotation:fake-annotation-a'],
      },
      state: {
        kind: 'item',
        total: 2,
        previousItem: null,
        nextItem: secondItem,
      },
    })
  })
})
