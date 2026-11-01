import { useRouterState } from '@tanstack/react-router'
import { useEffect, useState } from 'react'

import {
  type ReviewNavigationSnapshot,
  type ReviewNavigationTarget,
  type ReviewQueueItem,
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

export interface ScopedReviewNavigationScope {
  key: string
  items: ReviewQueueItem[] | undefined
  isPending: boolean
  isError: boolean
  scope: 'notes' | 'annotations'
}

export type NotesReviewNavigationScope = Omit<
  ScopedReviewNavigationScope,
  'scope'
> & { scope?: 'notes' }

type ReviewNavigationScope =
  | NotesReviewNavigationScope
  | (ScopedReviewNavigationScope & { scope: 'annotations' })

export function useReviewNavigation(
  currentItemKey: string,
  currentNoteId?: string,
  navigationScope?: ReviewNavigationScope,
): ReviewNavigationResult {
  const reviewQueue = useReviewQueue({ enabled: navigationScope == null })
  const historyState = useRouterState({
    select: (routerState) =>
      reviewQueueModel.readNavigationState(routerState.location.state),
  })
  const scopedHistoryItems =
    historyState?.scope != null &&
    (navigationScope == null || historyState.scope === navigationScope.scope) &&
    Array.isArray(historyState.items)
      ? historyState.items
      : null
  const currentNoteItem =
    historyState?.scope === 'notes' &&
    scopedHistoryItems != null &&
    currentNoteId != null
      ? scopedHistoryItems.find(
          (item) => item.kind === 'note' && item.noteId === currentNoteId,
        )
      : undefined
  const hasMatchingScopedHistoryState =
    historyState?.scope != null &&
    scopedHistoryItems != null &&
    (historyState.currentItemKey === currentItemKey || currentNoteItem != null)
  const matchingHistoryState = hasMatchingScopedHistoryState
    ? historyState
    : navigationScope == null && historyState?.currentItemKey === currentItemKey
      ? historyState
      : null
  const [fallbackNavigation, setFallbackNavigation] = useState<{
    currentItemKey: string
    scopeKey: string | null
    snapshot: ReviewNavigationSnapshot
    items: ReviewQueueItem[]
  } | null>(null)
  const fallbackSnapshot =
    fallbackNavigation?.currentItemKey === currentItemKey &&
    fallbackNavigation.scopeKey === (navigationScope?.key ?? null)
      ? fallbackNavigation.snapshot
      : null
  const fallbackItems =
    fallbackNavigation?.currentItemKey === currentItemKey &&
    fallbackNavigation.scopeKey === (navigationScope?.key ?? null)
      ? fallbackNavigation.items
      : null

  useEffect(() => {
    if (
      matchingHistoryState != null ||
      fallbackSnapshot != null ||
      (navigationScope == null ? reviewQueue.data : navigationScope.items) ==
        null
    ) {
      return
    }
    const items = navigationScope?.items ?? reviewQueue.data?.items
    if (items == null) return
    setFallbackNavigation({
      currentItemKey,
      scopeKey: navigationScope?.key ?? null,
      snapshot: reviewQueueModel.createNavigationSnapshot(items),
      items,
    })
  }, [
    currentItemKey,
    fallbackSnapshot,
    matchingHistoryState,
    navigationScope,
    reviewQueue.data,
  ])

  const snapshot = matchingHistoryState?.snapshot ?? fallbackSnapshot
  const isUsingScopedNavigation =
    hasMatchingScopedHistoryState || navigationScope != null
  const activeScope =
    navigationScope == null
      ? historyState?.scope
      : (navigationScope.scope ?? 'notes')
  const navigationItems = hasMatchingScopedHistoryState
    ? scopedHistoryItems
    : navigationScope != null
      ? (fallbackItems ?? navigationScope.items)
      : reviewQueue.data?.items
  const notesScopeCurrentItem =
    activeScope === 'notes' && currentNoteId != null
      ? navigationItems?.find(
          (item) => item.kind === 'note' && item.noteId === currentNoteId,
        )
      : undefined
  const navigationNoteItem = currentNoteItem ?? notesScopeCurrentItem

  return {
    navigation:
      snapshot == null || navigationItems == null
        ? null
        : {
            items: navigationItems,
            snapshot,
            currentItemKey:
              navigationNoteItem == null
                ? currentItemKey
                : reviewQueueItemKey(navigationNoteItem),
            ...(isUsingScopedNavigation && activeScope != null
              ? { scope: activeScope }
              : {}),
          },
    isPending: isUsingScopedNavigation
      ? (navigationItems == null &&
          (navigationScope?.isPending ?? reviewQueue.isPending)) ||
        (snapshot == null && !(navigationScope?.isError ?? reviewQueue.isError))
      : (navigationItems == null && reviewQueue.isPending) ||
        (snapshot == null && !reviewQueue.isError),
    isError: isUsingScopedNavigation
      ? navigationItems == null &&
        (navigationScope?.isError ?? reviewQueue.isError)
      : navigationItems == null && reviewQueue.isError,
    invalidate: reviewQueue.invalidate,
  }
}
