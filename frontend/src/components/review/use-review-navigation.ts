import { useRouterState } from '@tanstack/react-router'
import { useEffect, useState } from 'react'

import {
  type ReviewNavigationSnapshot,
  type ReviewNavigationTarget,
  reviewQueueItemKey,
  reviewQueueModel,
} from '#components/review/review-queue-model'
import { useReviewQueue } from '#components/review/use-review-queue'

export interface ReviewNavigationViewState {
  navigation: ReviewNavigationTarget | null
  isPending: boolean
  isError: boolean
}

export interface ReviewNavigationResult extends ReviewNavigationViewState {
  invalidate: () => void
}

export function useReviewNavigation(
  currentItemKey: string,
  currentNoteId?: string,
): ReviewNavigationResult {
  const reviewQueue = useReviewQueue()
  const historyState = useRouterState({
    select: (routerState) =>
      reviewQueueModel.readNavigationState(routerState.location.state),
  })
  const scopedHistoryItems =
    historyState?.scope === 'notes' && Array.isArray(historyState.items)
      ? historyState.items
      : null
  const currentNoteItem =
    scopedHistoryItems != null && currentNoteId != null
      ? scopedHistoryItems.find(
          (item) => item.kind === 'note' && item.noteId === currentNoteId,
        )
      : undefined
  const hasMatchingScopedHistoryState =
    historyState?.scope === 'notes' &&
    scopedHistoryItems != null &&
    (historyState.currentItemKey === currentItemKey || currentNoteItem != null)
  const matchingHistoryState =
    historyState != null &&
    (historyState.currentItemKey === currentItemKey ||
      hasMatchingScopedHistoryState)
      ? historyState
      : null
  const [fallbackNavigation, setFallbackNavigation] = useState<{
    currentItemKey: string
    snapshot: ReviewNavigationSnapshot
  } | null>(null)
  const fallbackSnapshot =
    fallbackNavigation?.currentItemKey === currentItemKey
      ? fallbackNavigation.snapshot
      : null

  useEffect(() => {
    if (
      matchingHistoryState != null ||
      fallbackSnapshot != null ||
      reviewQueue.data == null
    ) {
      return
    }
    setFallbackNavigation({
      currentItemKey,
      snapshot: reviewQueueModel.createNavigationSnapshot(
        reviewQueue.data.items,
      ),
    })
  }, [currentItemKey, fallbackSnapshot, matchingHistoryState, reviewQueue.data])

  const snapshot = matchingHistoryState?.snapshot ?? fallbackSnapshot
  const navigationItems = hasMatchingScopedHistoryState
    ? scopedHistoryItems
    : reviewQueue.data?.items

  return {
    navigation:
      snapshot == null || navigationItems == null
        ? null
        : {
            items: navigationItems,
            snapshot,
            currentItemKey:
              currentNoteItem == null
                ? currentItemKey
                : reviewQueueItemKey(currentNoteItem),
            ...(hasMatchingScopedHistoryState
              ? { scope: 'notes' as const }
              : {}),
          },
    isPending:
      (navigationItems == null && reviewQueue.isPending) ||
      (snapshot == null && !reviewQueue.isError),
    isError: navigationItems == null && reviewQueue.isError,
    invalidate: reviewQueue.invalidate,
  }
}
