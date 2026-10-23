import type { ReviewQueueItem } from '#components/review/review-queue-model'
import type { components } from '#lib/api/schema.gen'

export type NoteOverview = components['schemas']['NoteOverviewResponse']
export type NoteOverviewView = 'all' | 'pending'

function toReviewItem(
  overview: NoteOverview,
  version: components['schemas']['NoteVersion'],
): Extract<ReviewQueueItem, { kind: 'note' }> {
  return {
    kind: 'note',
    id: version.id,
    noteId: version.note_id,
    versionNo: version.version_no,
    title: version.title,
    createdAt: version.created_at,
    noteKind: overview.representative.kind ?? null,
    createdByKind: version.created_by_kind,
    unreadVersionCount: overview.unread_version_count,
  }
}

function createNoteOverviewReviewItem(
  overview: NoteOverview,
): Extract<ReviewQueueItem, { kind: 'note' }> | null {
  const latestUnread = overview.latest_unread
  return latestUnread == null ? null : toReviewItem(overview, latestUnread)
}

function createNoteOverviewNavigationItems(
  overviews: NoteOverview[],
  view: NoteOverviewView,
): Extract<ReviewQueueItem, { kind: 'note' }>[] {
  return overviews.map((overview) => {
    if (view === 'pending' && overview.latest_unread != null) {
      return toReviewItem(overview, overview.latest_unread)
    }
    return {
      kind: 'note',
      id: overview.representative.version_id,
      noteId: overview.representative.id,
      versionNo: overview.representative.version_no,
      title: overview.representative.title,
      createdAt: overview.representative.created_at,
      noteKind: overview.representative.kind ?? null,
      createdByKind: overview.representative.created_by_kind,
      unreadVersionCount: overview.unread_version_count,
    }
  })
}

function noteOverviewSelectionKey(noteId: string): string {
  return `note:${noteId}`
}

export const notesOverviewModel = {
  createNavigationItems: createNoteOverviewNavigationItems,
  createReviewItem: createNoteOverviewReviewItem,
  selectionKey: noteOverviewSelectionKey,
}
