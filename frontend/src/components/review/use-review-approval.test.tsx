import { act, renderHook } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'

import {
  type ReviewQueueData,
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

function getFrozenApprovalOutput(
  selectedItemIds: string[],
  selectedItems: ReviewQueueItem[],
  approvedItems: ReviewQueueItem[],
) {
  return {
    selectedBeforeApproval: {
      ids: selectedItemIds,
      items: selectedItems,
    },
    approvedItems,
  }
}

describe('useReviewApproval', () => {
  function startApprovalOfEveryItem() {
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
    return { approvals, approveItem, invalidate, result }
  }

  function runSecondApprovalFailure() {
    const attempt = startApprovalOfEveryItem()
    act(() => {
      attempt.approvals[0]?.callbacks.onSuccess()
    })
    act(() => {
      attempt.approvals[1]?.callbacks.onError()
    })
    return attempt
  }

  function runAllApprovalsSuccessfully() {
    const attempt = startApprovalOfEveryItem()
    for (let index = 0; index < items.length; index += 1) {
      act(() => {
        attempt.approvals[index]?.callbacks.onSuccess()
      })
    }
    return attempt
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

  it('attempts every selected item after successful approvals', () => {
    const { approveItem } = runAllApprovalsSuccessfully()

    expect(
      approveItem.mock.calls.map(([item]) => reviewQueueItemKey(item)),
    ).toEqual([
      'note:fake-version-a',
      'annotation:fake-annotation-b',
      'note:fake-version-c',
    ])
  })

  it('clears selection and resets the approval state after all approvals succeed', () => {
    const { result } = runAllApprovalsSuccessfully()

    expect(result.current.state).toEqual({
      selectedItemIds: [],
      isApproving: false,
      hasApprovalError: false,
    })
  })

  it('invalidates the review queue once after all approvals succeed', () => {
    const { invalidate } = runAllApprovalsSuccessfully()

    expect(invalidate.mock.calls).toEqual([[]])
  })

  it('keeps the selected note version when a newer unread version arrives', () => {
    const olderVersion: ReviewQueueItem = {
      kind: 'note',
      id: 'fake-version-old',
      noteId: 'fake-note-a',
      versionNo: 4,
      title: '架空の確認メモ A',
      createdAt: '2026-10-12T01:00:00Z',
      noteKind: '架空の種別',
      createdByKind: 'llm',
      unreadVersionCount: 1,
    }
    const newerVersion: ReviewQueueItem = {
      kind: 'note',
      id: 'fake-version-new',
      noteId: 'fake-note-a',
      versionNo: 5,
      title: '架空の確認メモ A',
      createdAt: '2026-10-13T01:00:00Z',
      noteKind: '架空の種別',
      createdByKind: 'llm',
      unreadVersionCount: 2,
    }
    const approveItem = vi.fn(
      (item: ReviewQueueItem, callbacks: ReviewApprovalCallbacks) =>
        void [item, callbacks],
    )
    const dataFor = (item: ReviewQueueItem): ReviewQueueData => ({
      counts: { notes: 1, annotations: 0, total: 1 },
      items: [item],
    })
    const getSelectionKey = (item: ReviewQueueItem) =>
      item.kind === 'note' ? `note:${item.noteId}` : reviewQueueItemKey(item)
    const { result, rerender } = renderHook(
      ({ data }: { data: ReviewQueueData }) =>
        useReviewApproval({
          data,
          approveItem,
          invalidate: () => {},
          getSelectionKey,
          preserveSelectedItems: true,
        }),
      { initialProps: { data: dataFor(olderVersion) } },
    )

    act(() => {
      result.current.actions.toggleItem('note:fake-note-a', true)
    })
    rerender({ data: dataFor(newerVersion) })

    act(() => {
      result.current.actions.approveSelected()
    })

    expect(
      getFrozenApprovalOutput(
        result.current.state.selectedItemIds,
        result.current.selectedItems,
        approveItem.mock.calls.map(([item]) => item),
      ),
    ).toEqual({
      selectedBeforeApproval: {
        ids: ['note:fake-note-a'],
        items: [olderVersion],
      },
      approvedItems: [olderVersion],
    })
  })
})
