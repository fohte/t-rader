import { useRouterState } from '@tanstack/react-router'
import { useEffect, useState } from 'react'

import {
  type ReviewNavigationSnapshot,
  type ReviewNavigationTarget,
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
): ReviewNavigationResult {
  const reviewQueue = useReviewQueue()
  const historyState = useRouterState({
    select: (routerState) =>
      reviewQueueModel.readNavigationState(routerState.location.state),
  })
  const hasMatchingHistoryState =
    historyState?.currentItemKey === currentItemKey
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
      hasMatchingHistoryState ||
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
  }, [
    currentItemKey,
    fallbackSnapshot,
    hasMatchingHistoryState,
    reviewQueue.data,
  ])

  const snapshot = hasMatchingHistoryState
    ? historyState.snapshot
    : fallbackSnapshot

  return {
    navigation:
      snapshot == null || reviewQueue.data == null
        ? null
        : {
            items: reviewQueue.data.items,
            snapshot,
            currentItemKey,
          },
    isPending:
      reviewQueue.isPending || (snapshot == null && !reviewQueue.isError),
    isError: reviewQueue.isError,
    invalidate: reviewQueue.invalidate,
  }
}
