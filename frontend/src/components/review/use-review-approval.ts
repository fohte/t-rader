import { useState } from 'react'

import {
  type ReviewQueueData,
  type ReviewQueueItem,
  reviewQueueItemKey,
} from '#components/review/review-queue-model'

export interface ReviewApprovalCallbacks {
  onSuccess: () => void
  onError: () => void
}

export interface UseReviewApprovalOptions {
  data: ReviewQueueData | null
  approveItem: (
    item: ReviewQueueItem,
    callbacks: ReviewApprovalCallbacks,
  ) => void
  invalidate: () => void
}

export function useReviewApproval({
  data,
  approveItem,
  invalidate,
}: UseReviewApprovalOptions) {
  const [selectedItemIds, setSelectedItemIds] = useState<string[]>([])
  const [isApproving, setIsApproving] = useState(false)
  const [hasApprovalError, setHasApprovalError] = useState(false)

  function approveInOrder(items: ReviewQueueItem[], index: number): void {
    const item = items[index]
    if (item == null) {
      setIsApproving(false)
      setSelectedItemIds([])
      invalidate()
      return
    }

    approveItem(item, {
      onSuccess: () => {
        const itemKey = reviewQueueItemKey(item)
        setSelectedItemIds((selected) =>
          selected.filter((selectedKey) => selectedKey !== itemKey),
        )
        approveInOrder(items, index + 1)
      },
      onError: () => {
        setIsApproving(false)
        setHasApprovalError(true)
        invalidate()
      },
    })
  }

  function approveSelected(): void {
    if (data == null || isApproving) return
    const selected = new Set(selectedItemIds)
    const selectedItems = data.items.filter((item) =>
      selected.has(reviewQueueItemKey(item)),
    )
    if (selectedItems.length === 0) return
    setHasApprovalError(false)
    setIsApproving(true)
    approveInOrder(selectedItems, 0)
  }

  function toggleItem(itemKey: string, checked: boolean): void {
    setSelectedItemIds((selected) =>
      checked
        ? [...new Set([...selected, itemKey])]
        : selected.filter((selectedKey) => selectedKey !== itemKey),
    )
  }

  function toggleVisibleItems(itemKeys: string[], checked: boolean): void {
    setSelectedItemIds((selected) =>
      checked
        ? [...new Set([...selected, ...itemKeys])]
        : selected.filter((selectedKey) => !itemKeys.includes(selectedKey)),
    )
  }

  function clearSelection(): void {
    setSelectedItemIds([])
  }

  return {
    state: { selectedItemIds, isApproving, hasApprovalError },
    actions: {
      approveSelected,
      toggleItem,
      toggleVisibleItems,
      clearSelection,
    },
  }
}
