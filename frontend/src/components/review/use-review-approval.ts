import { useCallback, useState } from 'react'

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
  getSelectionKey?: (item: ReviewQueueItem) => string
  preserveSelectedItems?: boolean
}

export function useReviewApproval({
  data,
  approveItem,
  invalidate,
  getSelectionKey = reviewQueueItemKey,
  preserveSelectedItems = false,
}: UseReviewApprovalOptions) {
  const [selectedItemIds, setSelectedItemIds] = useState<string[]>([])
  const [selectedItemsByKey, setSelectedItemsByKey] = useState<
    Map<string, ReviewQueueItem>
  >(() => new Map())
  const [isApproving, setIsApproving] = useState(false)
  const [hasApprovalError, setHasApprovalError] = useState(false)

  function approveInOrder(items: ReviewQueueItem[], index: number): void {
    const item = items[index]
    if (item == null) {
      setIsApproving(false)
      setSelectedItemIds([])
      setSelectedItemsByKey(new Map())
      invalidate()
      return
    }

    approveItem(item, {
      onSuccess: () => {
        const itemKey = getSelectionKey(item)
        setSelectedItemIds((selected) =>
          selected.filter((selectedKey) => selectedKey !== itemKey),
        )
        setSelectedItemsByKey((selected) => {
          const next = new Map(selected)
          next.delete(itemKey)
          return next
        })
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
    const items = preserveSelectedItems
      ? selectedItemIds.flatMap((key) => {
          const item = selectedItemsByKey.get(key)
          return item == null ? [] : [item]
        })
      : data.items.filter((item) => selected.has(getSelectionKey(item)))
    if (items.length === 0) return
    setHasApprovalError(false)
    setIsApproving(true)
    approveInOrder(items, 0)
  }

  function approveOne(itemKey: string): void {
    if (isApproving) return
    const item = selectedItemsByKey.get(itemKey)
    if (item == null) return
    setHasApprovalError(false)
    setIsApproving(true)
    approveInOrder([item], 0)
  }

  function toggleItem(itemKey: string, checked: boolean): void {
    if (preserveSelectedItems && checked) {
      const item = data?.items.find(
        (candidate) => getSelectionKey(candidate) === itemKey,
      )
      if (item == null) return
      setSelectedItemsByKey((selected) => {
        if (selected.has(itemKey)) return selected
        return new Map(selected).set(itemKey, item)
      })
    } else if (preserveSelectedItems) {
      setSelectedItemsByKey((selected) => {
        const next = new Map(selected)
        next.delete(itemKey)
        return next
      })
    }

    setSelectedItemIds((selected) =>
      checked
        ? [...new Set([...selected, itemKey])]
        : selected.filter((selectedKey) => selectedKey !== itemKey),
    )
  }

  function toggleVisibleItems(itemKeys: string[], checked: boolean): void {
    if (preserveSelectedItems) {
      setSelectedItemsByKey((selected) => {
        const next = new Map(selected)
        if (checked) {
          for (const itemKey of itemKeys) {
            if (next.has(itemKey)) continue
            const item = data?.items.find(
              (candidate) => getSelectionKey(candidate) === itemKey,
            )
            if (item != null) next.set(itemKey, item)
          }
        } else {
          for (const itemKey of itemKeys) next.delete(itemKey)
        }
        return next
      })
    }

    setSelectedItemIds((selected) =>
      checked
        ? [...new Set([...selected, ...itemKeys])]
        : selected.filter((selectedKey) => !itemKeys.includes(selectedKey)),
    )
  }

  const clearSelection = useCallback((): void => {
    setSelectedItemIds([])
    setSelectedItemsByKey(new Map())
  }, [])

  return {
    state: { selectedItemIds, isApproving, hasApprovalError },
    selectedItems: selectedItemIds.flatMap((key) => {
      const item = selectedItemsByKey.get(key)
      return item == null ? [] : [item]
    }),
    actions: {
      approveSelected,
      approveOne,
      toggleItem,
      toggleVisibleItems,
      clearSelection,
    },
  }
}
