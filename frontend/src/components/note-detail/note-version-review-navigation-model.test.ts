import { describe, expect, it } from 'vitest'

import { createNoteVersionReviewDestination } from '#components/note-detail/note-version-review-navigation-model'
import type {
  ReviewNavigationSnapshot,
  ReviewQueueItem,
} from '#components/review/review-queue-model'

const snapshot: ReviewNavigationSnapshot = {
  itemKeys: ['note:fake-version-a', 'annotation:fake-annotation-a'],
}

describe('createNoteVersionReviewDestination', () => {
  it('preserves the note version and opened queue state', () => {
    const item: ReviewQueueItem = {
      kind: 'note',
      id: 'fake-version-a',
      noteId: 'fake-note-a',
      versionNo: 2,
      title: '架空データの確認メモ',
      createdAt: '2026-01-12T02:00:00Z',
      noteKind: '架空の種別',
      createdByKind: 'llm',
      unreadVersionCount: 1,
    }

    expect(createNoteVersionReviewDestination(item, snapshot)).toEqual({
      to: '/notes/$noteId',
      params: { noteId: 'fake-note-a' },
      search: { version: 2, version_id: 'fake-version-a' },
      reviewQueue: {
        snapshot,
        currentItemKey: 'note:fake-version-a',
      },
    })
  })

  it('preserves the annotation and opened queue state', () => {
    const item: ReviewQueueItem = {
      kind: 'annotation',
      id: 'fake-annotation-a',
      title: '架空データの注記',
      createdAt: '2026-01-12T01:00:00Z',
      targetSymbol: 'demo-code',
      timestamp: '2026-01-12T01:00:00Z',
      price: null,
    }

    expect(createNoteVersionReviewDestination(item, snapshot)).toEqual({
      to: '/annotations/$annoId',
      params: { annoId: 'fake-annotation-a' },
      reviewQueue: {
        snapshot,
        currentItemKey: 'annotation:fake-annotation-a',
      },
    })
  })
})
