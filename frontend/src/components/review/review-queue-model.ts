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
      createdByKind: string
      unreadVersionCount: number
      warningDispositions?: components['schemas']['WarningDispositionResponse'][]
    }
  | {
      kind: 'annotation'
      id: string
      title: string
      createdAt: string
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

export function reviewQueueItemKey(
  item: Pick<ReviewQueueItem, 'kind' | 'id'>,
): string {
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

function createAnnotationReviewItems(
  annotations: Annotation[],
): Extract<ReviewQueueItem, { kind: 'annotation' }>[] {
  return annotations.map((annotation) => ({
    kind: 'annotation',
    id: annotation.id,
    title: annotation.text,
    createdAt: annotation.created_at,
    targetSymbol: annotation.target_symbol,
    timestamp: annotation.timestamp,
    price: annotation.price ?? null,
  }))
}

function buildReviewQueueData({
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
        createdByKind: latest.created_by_kind,
        unreadVersionCount: versions.length,
        ...(latest.warning_dispositions.length > 0
          ? { warningDispositions: latest.warning_dispositions }
          : {}),
      }
    },
  )
  const unreadAnnotations: ReviewQueueItem[] = createAnnotationReviewItems(
    annotations.filter((annotation) => annotation.status === 'unread'),
  )
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

export interface ReviewQueueNavigationState {
  snapshot: ReviewNavigationSnapshot
  currentItemKey: string
  items?: ReviewQueueItem[]
  scope?: 'notes' | 'annotations'
}

export type ReviewNavigationTarget = ReviewQueueNavigationState & {
  items: ReviewQueueItem[]
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null
}

function isStringArray(value: unknown): value is string[] {
  return Array.isArray(value) && value.every((item) => typeof item === 'string')
}

function isNoteReviewItem(
  value: unknown,
): value is Extract<ReviewQueueItem, { kind: 'note' }> {
  if (!isRecord(value) || value.kind !== 'note') return false
  return (
    typeof value.id === 'string' &&
    typeof value.noteId === 'string' &&
    typeof value.versionNo === 'number' &&
    typeof value.title === 'string' &&
    typeof value.createdAt === 'string' &&
    (typeof value.noteKind === 'string' || value.noteKind === null) &&
    typeof value.createdByKind === 'string' &&
    typeof value.unreadVersionCount === 'number'
  )
}

function isNoteReviewItemArray(value: unknown): value is ReviewQueueItem[] {
  return Array.isArray(value) && value.every(isNoteReviewItem)
}

function isAnnotationReviewItem(
  value: unknown,
): value is Extract<ReviewQueueItem, { kind: 'annotation' }> {
  if (!isRecord(value) || value.kind !== 'annotation') return false
  return (
    typeof value.id === 'string' &&
    typeof value.title === 'string' &&
    typeof value.createdAt === 'string' &&
    typeof value.targetSymbol === 'string' &&
    typeof value.timestamp === 'string' &&
    (typeof value.price === 'number' || value.price === null)
  )
}

function isAnnotationReviewItemArray(
  value: unknown,
): value is ReviewQueueItem[] {
  return Array.isArray(value) && value.every(isAnnotationReviewItem)
}

function isReviewQueueNavigationState(
  value: unknown,
): value is ReviewQueueNavigationState {
  if (!isRecord(value) || !isRecord(value.snapshot)) return false
  const hasSnapshot =
    isStringArray(value.snapshot.itemKeys) &&
    typeof value.currentItemKey === 'string'
  if (!hasSnapshot) return false
  if (value.scope === 'notes') return isNoteReviewItemArray(value.items)
  if (value.scope === 'annotations') {
    return isAnnotationReviewItemArray(value.items)
  }
  return value.scope == null && value.items == null
}

function readReviewQueueNavigationState(
  state: unknown,
): ReviewQueueNavigationState | null {
  if (!isRecord(state)) return null
  const reviewQueue = state.reviewQueue
  return isReviewQueueNavigationState(reviewQueue) ? reviewQueue : null
}

function createReviewNavigationSnapshot(
  items: ReviewQueueItem[],
): ReviewNavigationSnapshot {
  return { itemKeys: Object.freeze(items.map(reviewQueueItemKey)) }
}

type ReviewNavigation =
  | { kind: 'empty'; total: 0 }
  | { kind: 'start'; total: number; firstItem: ReviewQueueItem | null }
  | {
      kind: 'item'
      total: number
      previousItem: ReviewQueueItem | null
      nextItem: ReviewQueueItem | null
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

function getReviewNavigation(
  snapshot: ReviewNavigationSnapshot,
  currentItemKey: string,
  remainingItems: ReviewQueueItem[],
): ReviewNavigation {
  const firstRemainingItem = remainingItems.at(0)
  if (firstRemainingItem == null) return { kind: 'empty', total: 0 }

  const firstOtherItem =
    remainingItems.find(
      (item) => reviewQueueItemKey(item) !== currentItemKey,
    ) ?? null
  const currentIndex = snapshot.itemKeys.indexOf(currentItemKey)
  if (currentIndex === -1) {
    return {
      kind: 'start',
      total: remainingItems.length,
      firstItem: firstOtherItem,
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
  const nextItemWithoutCurrent =
    nextItem != null && reviewQueueItemKey(nextItem) !== currentItemKey
      ? nextItem
      : firstOtherItem
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
    nextItem: nextItemWithoutCurrent,
  }
}

export const reviewQueueModel = {
  buildData: buildReviewQueueData,
  createAnnotationItems: createAnnotationReviewItems,
  createNavigationSnapshot: createReviewNavigationSnapshot,
  getNavigation: getReviewNavigation,
  readNavigationState: readReviewQueueNavigationState,
}
