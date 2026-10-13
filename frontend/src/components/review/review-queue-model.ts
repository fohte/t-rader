import type { components } from '#lib/api/schema.gen'

type Annotation = components['schemas']['Annotation']
type PendingNoteVersion = components['schemas']['PendingNoteVersion']

export type ReviewQueueItem =
  | {
      kind: 'note'
      id: string
      noteId: string
      versionNo: number
      title: string
      createdAt: string
      noteKind: string | null
      tags: string[]
      changeReason: string | null
      createdByKind: string
      unreadVersionCount: number
    }
  | {
      kind: 'annotation'
      id: string
      title: string
      createdAt: string
      targetKind: string
      targetSymbol: string
      timestamp: string
      price: number | null
    }

export interface ReviewQueueData {
  counts: {
    notes: number
    annotations: number
    total: number
  }
  items: ReviewQueueItem[]
}

export function reviewQueueItemKey(item: ReviewQueueItem): string {
  return `${item.kind}:${item.id}`
}

function compareReviewItems(
  left: ReviewQueueItem,
  right: ReviewQueueItem,
): number {
  return (
    right.createdAt.localeCompare(left.createdAt) ||
    reviewQueueItemKey(left).localeCompare(reviewQueueItemKey(right))
  )
}

export function buildReviewQueueData({
  noteVersions,
  annotations,
}: {
  noteVersions: PendingNoteVersion[]
  annotations: Annotation[]
}): ReviewQueueData {
  const unreadVersionsByNote = new Map<string, PendingNoteVersion[]>()
  for (const version of noteVersions) {
    if (version.status !== 'unread') continue
    const versions = unreadVersionsByNote.get(version.note_id) ?? []
    versions.push(version)
    unreadVersionsByNote.set(version.note_id, versions)
  }

  const noteItems: ReviewQueueItem[] = [...unreadVersionsByNote.values()].map(
    (versions) => {
      const latest = versions.reduce((currentLatest, candidate) =>
        candidate.created_at.localeCompare(currentLatest.created_at) > 0 ||
        (candidate.created_at === currentLatest.created_at &&
          candidate.version_no > currentLatest.version_no)
          ? candidate
          : currentLatest,
      )
      return {
        kind: 'note',
        id: latest.id,
        noteId: latest.note_id,
        versionNo: latest.version_no,
        title: latest.title,
        createdAt: latest.created_at,
        noteKind: latest.kind ?? null,
        tags: latest.tags,
        changeReason: latest.change_reason ?? null,
        createdByKind: latest.created_by_kind,
        unreadVersionCount: versions.length,
      }
    },
  )
  const unreadAnnotations: ReviewQueueItem[] = annotations
    .filter((annotation) => annotation.status === 'unread')
    .map((annotation) => ({
      kind: 'annotation',
      id: annotation.id,
      title: annotation.text,
      createdAt: annotation.created_at,
      targetKind: annotation.target_kind,
      targetSymbol: annotation.target_symbol,
      timestamp: annotation.timestamp,
      price: annotation.price ?? null,
    }))
  const items = [...noteItems, ...unreadAnnotations].sort(compareReviewItems)

  return {
    counts: {
      notes: noteItems.length,
      annotations: unreadAnnotations.length,
      total: items.length,
    },
    items,
  }
}

export interface ReviewNavigationSnapshot {
  itemKeys: readonly string[]
}

export function createReviewNavigationSnapshot(
  items: ReviewQueueItem[],
): ReviewNavigationSnapshot {
  return { itemKeys: Object.freeze(items.map(reviewQueueItemKey)) }
}

export type ReviewNavigation =
  | { kind: 'empty'; total: 0 }
  | { kind: 'start'; total: number; firstItem: ReviewQueueItem }
  | {
      kind: 'item'
      total: number
      previousItem: ReviewQueueItem | null
      nextItem: ReviewQueueItem
    }

export interface ReviewNavigationState {
  snapshot: ReviewNavigationSnapshot
  currentItemKey: string
  navigation: ReviewNavigation
}

function findRemainingItem(
  snapshot: ReviewNavigationSnapshot,
  startIndex: number,
  direction: -1 | 1,
  remainingItemsByKey: ReadonlyMap<string, ReviewQueueItem>,
  wrap: boolean,
): ReviewQueueItem | null {
  const { itemKeys } = snapshot
  for (let offset = 1; offset <= itemKeys.length; offset += 1) {
    const candidateIndex = startIndex + direction * offset
    if (!wrap && (candidateIndex < 0 || candidateIndex >= itemKeys.length)) {
      return null
    }
    const index = (candidateIndex + itemKeys.length) % itemKeys.length
    const candidateKey = itemKeys[index]
    if (candidateKey == null) continue
    const item = remainingItemsByKey.get(candidateKey)
    if (item != null) return item
  }
  return null
}

export function getReviewNavigation(
  snapshot: ReviewNavigationSnapshot,
  currentItemKey: string,
  remainingItems: ReviewQueueItem[],
): ReviewNavigation {
  const firstRemainingItem = remainingItems.at(0)
  if (firstRemainingItem == null) return { kind: 'empty', total: 0 }

  const currentIndex = snapshot.itemKeys.indexOf(currentItemKey)
  if (currentIndex === -1) {
    return {
      kind: 'start',
      total: remainingItems.length,
      firstItem: firstRemainingItem,
    }
  }

  const remainingItemsByKey = new Map(
    remainingItems.map((item) => [reviewQueueItemKey(item), item]),
  )
  const nextItem = findRemainingItem(
    snapshot,
    currentIndex,
    1,
    remainingItemsByKey,
    true,
  )
  return {
    kind: 'item',
    total: remainingItems.length,
    previousItem: findRemainingItem(
      snapshot,
      currentIndex,
      -1,
      remainingItemsByKey,
      false,
    ),
    nextItem: nextItem ?? firstRemainingItem,
  }
}
