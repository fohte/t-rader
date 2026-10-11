import type { components } from '#lib/api/schema.gen'

type Annotation = components['schemas']['Annotation']
type Note = components['schemas']['Note']
type NoteVersion = components['schemas']['NoteVersion']

interface ReviewQueueCounts {
  notes: number
  noteVersions: number
  annotations: number
  total: number
}

export type ReviewQueueItem =
  | {
      kind: 'note'
      id: string
      noteId: string
      title: string
      createdAt: string
    }
  | {
      kind: 'note-version'
      id: string
      noteId: string
      versionNo: number
      title: string
      createdAt: string
    }
  | {
      kind: 'annotation'
      id: string
      title: string
      createdAt: string
      targetSymbol: string
    }

export interface ReviewQueueData {
  counts: ReviewQueueCounts
  items: ReviewQueueItem[]
}

export function buildReviewQueueData({
  notes,
  noteVersions,
  annotations,
}: {
  notes: Note[]
  noteVersions: NoteVersion[]
  annotations: Annotation[]
}): ReviewQueueData {
  const unreadAnnotations = annotations.filter(
    (annotation) => annotation.status === 'unread',
  )
  // 未読ノートと承認待ち一覧は同じバージョンを返すことがあるため、項目数は重複させない。
  const pendingVersionIds = new Set(noteVersions.map((version) => version.id))
  const items: ReviewQueueItem[] = [
    ...notes
      .filter((note) => !pendingVersionIds.has(note.version_id))
      .map((note) => ({
        kind: 'note' as const,
        id: note.id,
        noteId: note.id,
        title: note.title,
        createdAt: note.created_at,
      })),
    ...noteVersions.map((version) => ({
      kind: 'note-version' as const,
      id: version.id,
      noteId: version.note_id,
      versionNo: version.version_no,
      title: version.title,
      createdAt: version.created_at,
    })),
    ...unreadAnnotations.map((annotation) => ({
      kind: 'annotation' as const,
      id: annotation.id,
      title: annotation.text,
      createdAt: annotation.created_at,
      targetSymbol: annotation.target_symbol,
    })),
  ].sort((left, right) => right.createdAt.localeCompare(left.createdAt))

  return {
    counts: {
      notes: notes.length,
      noteVersions: noteVersions.length,
      annotations: unreadAnnotations.length,
      total: items.length,
    },
    items: items.slice(0, 3),
  }
}
