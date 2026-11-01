import { describe, expect, it } from 'vitest'

import {
  type ReviewQueueItem,
  reviewQueueModel,
} from '#components/review/review-queue-model'
import type { components } from '#lib/api/schema.gen'

type Annotation = components['schemas']['Annotation']
type PendingNoteVersion = components['schemas']['PendingNoteVersion']

function createNoteVersion(
  id: string,
  noteId: string,
  versionNo: number,
  createdAt: string,
): PendingNoteVersion {
  return {
    id,
    note_id: noteId,
    version_no: versionNo,
    title: `架空のノート v${String(versionNo)}`,
    body_md: '架空の本文です。',
    frontmatter_json: {},
    resolved_price_references_json: {},
    warning_dispositions: [],
    graphs_json: [],
    status: 'unread',
    is_current: true,
    created_by_kind: 'llm',
    created_at: createdAt,
    kind: '架空の種別',
    tags: ['架空タグ'],
  }
}

function createAnnotation(
  id: string,
  status: string,
  createdAt: string,
): Annotation {
  return {
    id,
    created_by_kind: 'human',
    status,
    text: '架空の確認メモ',
    target_kind: 'stock',
    target_symbol: 'DEMO-A',
    timestamp: createdAt,
    created_at: createdAt,
    updated_at: createdAt,
  }
}

function createAnnotationItem(id: string, createdAt: string): ReviewQueueItem {
  return {
    kind: 'annotation',
    id,
    title: '架空の確認メモ',
    createdAt,
    targetSymbol: 'DEMO-A',
    timestamp: createdAt,
    price: null,
  }
}

describe('buildReviewQueueData', () => {
  it('keeps the newest unread version for each note and orders it with unread annotations', () => {
    expect(
      reviewQueueModel.buildData({
        noteVersions: [
          createNoteVersion(
            'fake-version-old',
            'fake-note-a',
            2,
            '2026-10-10T01:00:00Z',
          ),
          createNoteVersion(
            'fake-version-b',
            'fake-note-b',
            1,
            '2026-10-11T01:00:00Z',
          ),
          createNoteVersion(
            'fake-version-new',
            'fake-note-a',
            3,
            '2026-10-12T01:00:00Z',
          ),
        ],
        annotations: [
          createAnnotation(
            'fake-annotation-a',
            'unread',
            '2026-10-11T12:00:00Z',
          ),
          createAnnotation(
            'fake-annotation-approved',
            'approved',
            '2026-10-13T01:00:00Z',
          ),
        ],
      }),
    ).toEqual({
      counts: { notes: 2, annotations: 1, total: 3 },
      items: [
        {
          kind: 'note',
          id: 'fake-version-new',
          noteId: 'fake-note-a',
          versionNo: 3,
          title: '架空のノート v3',
          createdAt: '2026-10-12T01:00:00Z',
          noteKind: '架空の種別',
          createdByKind: 'llm',
          unreadVersionCount: 2,
        },
        {
          kind: 'annotation',
          id: 'fake-annotation-a',
          title: '架空の確認メモ',
          createdAt: '2026-10-11T12:00:00Z',
          targetSymbol: 'DEMO-A',
          timestamp: '2026-10-11T12:00:00Z',
          price: null,
        },
        {
          kind: 'note',
          id: 'fake-version-b',
          noteId: 'fake-note-b',
          versionNo: 1,
          title: '架空のノート v1',
          createdAt: '2026-10-11T01:00:00Z',
          noteKind: '架空の種別',
          createdByKind: 'llm',
          unreadVersionCount: 1,
        },
      ],
    })
  })
})

describe('getReviewNavigation', () => {
  it('uses the opened order after the current item leaves the unread queue and wraps forward', () => {
    const items: ReviewQueueItem[] = [
      {
        kind: 'note',
        id: 'fake-version-a',
        noteId: 'fake-note-a',
        versionNo: 1,
        title: '架空のノート A',
        createdAt: '2026-10-12T01:00:00Z',
        noteKind: null,
        createdByKind: 'llm',
        unreadVersionCount: 1,
      },
      createAnnotationItem('fake-annotation-b', '2026-10-11T01:00:00Z'),
      {
        kind: 'note',
        id: 'fake-version-c',
        noteId: 'fake-note-c',
        versionNo: 1,
        title: '架空のノート C',
        createdAt: '2026-10-10T01:00:00Z',
        noteKind: null,
        createdByKind: 'llm',
        unreadVersionCount: 1,
      },
    ]
    const snapshot = reviewQueueModel.createNavigationSnapshot(items)

    expect(
      reviewQueueModel.getNavigation(
        snapshot,
        'note:fake-version-a',
        items.slice(1),
      ),
    ).toEqual({
      kind: 'item',
      total: 2,
      previousItem: null,
      nextItem: items[1],
    })
  })

  it('wraps from the last unread item to the first in the opened order', () => {
    const items: ReviewQueueItem[] = [
      createAnnotationItem('fake-annotation-a', '2026-10-12T01:00:00Z'),
      createAnnotationItem('fake-annotation-b', '2026-10-11T01:00:00Z'),
    ]
    const snapshot = reviewQueueModel.createNavigationSnapshot(items)

    expect(
      reviewQueueModel.getNavigation(
        snapshot,
        'annotation:fake-annotation-b',
        items,
      ),
    ).toEqual({
      kind: 'item',
      total: 2,
      previousItem: items[0],
      nextItem: items[0],
    })
  })

  it('starts at the first unread item when the current item was not in the opened queue', () => {
    const item = createAnnotationItem(
      'fake-annotation-a',
      '2026-10-12T01:00:00Z',
    )
    const openedItem = createAnnotationItem(
      'fake-annotation-in-queue',
      '2026-10-11T01:00:00Z',
    )
    const snapshot = reviewQueueModel.createNavigationSnapshot([openedItem])

    expect(
      reviewQueueModel.getNavigation(
        snapshot,
        'note:fake-version-outside-queue',
        [item],
      ),
    ).toEqual({ kind: 'start', total: 1, firstItem: item })
  })

  it('does not point to the current item when it is the only unread item', () => {
    const item = createAnnotationItem(
      'fake-annotation-a',
      '2026-10-12T01:00:00Z',
    )
    const snapshot = reviewQueueModel.createNavigationSnapshot([item])

    expect(
      reviewQueueModel.getNavigation(snapshot, 'annotation:fake-annotation-a', [
        item,
      ]),
    ).toEqual({
      kind: 'item',
      total: 1,
      previousItem: null,
      nextItem: null,
    })
  })

  it('does not link to the current item when it is outside the opened snapshot', () => {
    const item = createAnnotationItem(
      'fake-annotation-a',
      '2026-10-12T01:00:00Z',
    )
    const snapshot = reviewQueueModel.createNavigationSnapshot([])

    expect(
      reviewQueueModel.getNavigation(snapshot, 'annotation:fake-annotation-a', [
        item,
      ]),
    ).toEqual({ kind: 'start', total: 1, firstItem: null })
  })

  it('returns an empty state when there are no unread items', () => {
    const snapshot = reviewQueueModel.createNavigationSnapshot([
      createAnnotationItem('fake-annotation-a', '2026-10-12T01:00:00Z'),
    ])

    expect(
      reviewQueueModel.getNavigation(
        snapshot,
        'annotation:fake-annotation-a',
        [],
      ),
    ).toEqual({ kind: 'empty', total: 0 })
  })

  it('copies the order at snapshot creation', () => {
    const items = [
      createAnnotationItem('fake-annotation-a', '2026-10-12T01:00:00Z'),
      createAnnotationItem('fake-annotation-b', '2026-10-11T01:00:00Z'),
    ]
    const snapshot = reviewQueueModel.createNavigationSnapshot(items)
    items.reverse()

    expect(snapshot.itemKeys).toEqual([
      'annotation:fake-annotation-a',
      'annotation:fake-annotation-b',
    ])
  })
})
