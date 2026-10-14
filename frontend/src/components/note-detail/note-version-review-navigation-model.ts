import {
  type ReviewNavigationSnapshot,
  type ReviewQueueItem,
  reviewQueueItemKey,
  type ReviewQueueNavigationState,
} from '#components/review/review-queue-model'

export type NoteVersionReviewDestination =
  | {
      to: '/notes/$noteId'
      params: { noteId: string }
      search: { version: number; version_id: string }
      reviewQueue: ReviewQueueNavigationState
    }
  | {
      to: '/annotations/$annoId'
      params: { annoId: string }
      reviewQueue: ReviewQueueNavigationState
    }

export function createNoteVersionReviewDestination(
  item: ReviewQueueItem,
  snapshot: ReviewNavigationSnapshot,
): NoteVersionReviewDestination {
  const reviewQueue = {
    snapshot,
    currentItemKey: reviewQueueItemKey(item),
  }

  if (item.kind === 'note') {
    return {
      to: '/notes/$noteId',
      params: { noteId: item.noteId },
      search: { version: item.versionNo, version_id: item.id },
      reviewQueue,
    }
  }

  return {
    to: '/annotations/$annoId',
    params: { annoId: item.id },
    reviewQueue,
  }
}
