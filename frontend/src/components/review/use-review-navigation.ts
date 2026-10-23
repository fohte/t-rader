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

export interface NotesReviewNavigationScope {
  key: string
  items: ReviewQueueItem[] | undefined
  isPending: boolean
  isError: boolean
}

export function useReviewNavigation(
  currentItemKey: string,
  currentNoteId?: string,
  notesScope?: NotesReviewNavigationScope,
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
  const matchingHistoryState = hasMatchingScopedHistoryState
    ? historyState
    : notesScope == null && historyState?.currentItemKey === currentItemKey
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
    fallbackNavigation.scopeKey === (notesScope?.key ?? null)
      ? fallbackNavigation.snapshot
      : null
  const fallbackItems =
    fallbackNavigation?.currentItemKey === currentItemKey &&
    fallbackNavigation.scopeKey === (notesScope?.key ?? null)
      ? fallbackNavigation.items
      : null

  useEffect(() => {
    if (
      matchingHistoryState != null ||
      fallbackSnapshot != null ||
      (notesScope == null ? reviewQueue.data : notesScope.items) == null
    ) {
      return
    }
    const items = notesScope?.items ?? reviewQueue.data?.items
    if (items == null) return
    setFallbackNavigation({
      currentItemKey,
      scopeKey: notesScope?.key ?? null,
      snapshot: reviewQueueModel.createNavigationSnapshot(items),
      items,
    })
  }, [
    currentItemKey,
    fallbackSnapshot,
    matchingHistoryState,
    notesScope,
    reviewQueue.data,
  ])

  const snapshot = matchingHistoryState?.snapshot ?? fallbackSnapshot
  const isUsingNotesScope = hasMatchingScopedHistoryState || notesScope != null
  const navigationItems = hasMatchingScopedHistoryState
    ? scopedHistoryItems
    : notesScope != null
      ? (fallbackItems ?? notesScope.items)
      : reviewQueue.data?.items
  const notesScopeCurrentItem =
    isUsingNotesScope && currentNoteId != null
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
            ...(isUsingNotesScope ? { scope: 'notes' as const } : {}),
          },
    isPending: isUsingNotesScope
      ? (navigationItems == null &&
          (notesScope?.isPending ?? reviewQueue.isPending)) ||
        (snapshot == null && !(notesScope?.isError ?? reviewQueue.isError))
      : (navigationItems == null && reviewQueue.isPending) ||
        (snapshot == null && !reviewQueue.isError),
    isError: isUsingNotesScope
      ? navigationItems == null && (notesScope?.isError ?? reviewQueue.isError)
      : navigationItems == null && reviewQueue.isError,
    invalidate: reviewQueue.invalidate,
  }
}
