import { act, renderHook } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'

import {
  type ReviewQueueItem,
  reviewQueueItemKey,
} from '#components/review/review-queue-model'
import {
  type ReviewApprovalCallbacks,
  useReviewApproval,
} from '#components/review/use-review-approval'

const items: ReviewQueueItem[] = [
  {
    kind: 'note',
    id: 'fake-version-a',
    noteId: 'fake-note-a',
    versionNo: 2,
    title: '架空の確認メモ A',
    createdAt: '2026-10-12T01:00:00Z',
    noteKind: '架空の種別',
    createdByKind: 'llm',
    unreadVersionCount: 1,
  },
  {
    kind: 'annotation',
    id: 'fake-annotation-b',
    title: '架空の確認メモ B',
    createdAt: '2026-10-11T01:00:00Z',
    targetSymbol: 'DEMO-A',
    timestamp: '2026-10-11T01:00:00Z',
    price: null,
  },
  {
    kind: 'note',
    id: 'fake-version-c',
    noteId: 'fake-note-c',
    versionNo: 1,
    title: '架空の確認メモ C',
    createdAt: '2026-10-10T01:00:00Z',
    noteKind: null,
    createdByKind: 'llm',
    unreadVersionCount: 1,
  },
]

describe('useReviewApproval', () => {
  function runSecondApprovalFailure() {
    const approvals: Array<{
      item: ReviewQueueItem
      callbacks: ReviewApprovalCallbacks
    }> = []
    const approveItem = vi.fn(
      (item: ReviewQueueItem, callbacks: ReviewApprovalCallbacks) => {
        approvals.push({ item, callbacks })
      },
    )
    const invalidate = vi.fn()
    const { result } = renderHook(() =>
      useReviewApproval({
        data: {
          counts: { notes: 2, annotations: 1, total: 3 },
          items,
        },
        approveItem,
        invalidate,
      }),
    )
    const itemKeys = items.map(reviewQueueItemKey)

    act(() => {
      result.current.actions.toggleVisibleItems(itemKeys, true)
    })
    act(() => {
      result.current.actions.approveSelected()
    })
    act(() => {
      approvals[0]?.callbacks.onSuccess()
    })
    act(() => {
      approvals[1]?.callbacks.onError()
    })

    return { approvals, approveItem, invalidate, result }
  }

  it('stops after the first failed approval', () => {
    const { approveItem } = runSecondApprovalFailure()

    expect(
      approveItem.mock.calls.map(([item]) => reviewQueueItemKey(item)),
    ).toEqual(['note:fake-version-a', 'annotation:fake-annotation-b'])
  })

  it('keeps the failed and unprocessed items selected and reports the failure', () => {
    const { result } = runSecondApprovalFailure()

    expect(result.current.state).toEqual({
      selectedItemIds: ['annotation:fake-annotation-b', 'note:fake-version-c'],
      isApproving: false,
      hasApprovalError: true,
    })
  })

  it('invalidates the review queue after a failed approval', () => {
    const { invalidate } = runSecondApprovalFailure()

    expect(invalidate.mock.calls).toEqual([[]])
  })
})
