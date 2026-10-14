import { useEffect, useState } from 'react'

import {
  type ReviewNavigationSnapshot,
  type ReviewQueueItem,
  reviewQueueItemKey,
  reviewQueueModel,
  type ReviewQueueNavigationState,
} from '#components/review/review-queue-model'

export type ReviewQueueNavigationDisplayState =
  | { kind: 'loading' }
  | { kind: 'error' }
  | { kind: 'only-current'; total: 1 }
  | ReturnType<typeof reviewQueueModel.getNavigation>

interface UseReviewQueueNavigationOptions {
  scopeKey: string
  openedState?: ReviewQueueNavigationState
  currentItemKey: string
  items: ReviewQueueItem[] | undefined
  isError: boolean
}

export function useReviewQueueNavigation({
  scopeKey,
  openedState,
  currentItemKey,
  items,
  isError,
}: UseReviewQueueNavigationOptions): {
  snapshot: ReviewNavigationSnapshot | null
  state: ReviewQueueNavigationDisplayState
} {
  const [fallbackNavigation, setFallbackNavigation] = useState<{
    scopeKey: string
    snapshot: ReviewNavigationSnapshot | null
  }>(() => ({ scopeKey, snapshot: null }))
  const activeFallbackSnapshot =
    fallbackNavigation.scopeKey === scopeKey
      ? fallbackNavigation.snapshot
      : null
  const snapshot =
    openedState?.snapshot ??
    activeFallbackSnapshot ??
    (items == null ? null : reviewQueueModel.createNavigationSnapshot(items))

  useEffect(() => {
    if (
      openedState != null ||
      activeFallbackSnapshot != null ||
      items == null
    ) {
      return
    }

    setFallbackNavigation({
      scopeKey,
      snapshot: reviewQueueModel.createNavigationSnapshot(items),
    })
  }, [activeFallbackSnapshot, items, openedState, scopeKey])

  const activeCurrentItemKey = openedState?.currentItemKey ?? currentItemKey
  const navigation =
    items == null || snapshot == null
      ? null
      : reviewQueueModel.getNavigation(snapshot, activeCurrentItemKey, items)
  const currentIsOnlyRemainingItem =
    navigation?.total === 1 &&
    ((navigation.kind === 'item' &&
      reviewQueueItemKey(navigation.nextItem) === activeCurrentItemKey) ||
      (navigation.kind === 'start' &&
        reviewQueueItemKey(navigation.firstItem) === activeCurrentItemKey))
  const state = isError
    ? ({ kind: 'error' } as const)
    : currentIsOnlyRemainingItem
      ? ({ kind: 'only-current', total: 1 } as const)
      : (navigation ?? { kind: 'loading' as const })

  return { snapshot, state }
}
