import { describe, expect, it } from 'vitest'

import { buildReviewQueueData } from '#components/dashboard/dashboard-model'
import type { components } from '#lib/api/schema.gen'

type Annotation = components['schemas']['Annotation']
type Note = components['schemas']['Note']
type NoteVersion = components['schemas']['NoteVersion']

function createNote(
  id: string,
  versionId: string,
  title: string,
  createdAt: string,
): Note {
  return {
    id,
    version_id: versionId,
    version_no: 1,
    is_current: true,
    title,
    body_md: '架空の本文です。',
    frontmatter_json: {},
    resolved_price_references_json: {},
    graphs_json: [],
    tags: [],
    kind: null,
    status: 'unread',
    trigger: null,
    trigger_label: null,
    created_by_kind: 'llm',
    created_at: createdAt,
    updated_at: createdAt,
  }
}

function createNoteVersion(
  id: string,
  noteId: string,
  title: string,
  createdAt: string,
): NoteVersion {
  return {
    id,
    note_id: noteId,
    version_no: 1,
    title,
    body_md: '架空の本文です。',
    frontmatter_json: {},
    resolved_price_references_json: {},
    graphs_json: [],
    status: 'unread',
    is_current: true,
    created_by_kind: 'llm',
    created_at: createdAt,
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
    text: '価格帯を確認する',
    target_kind: 'stock',
    target_symbol: '0000',
    timestamp: createdAt,
    created_at: createdAt,
    updated_at: createdAt,
  }
}

describe('buildReviewQueueData', () => {
  it('counts unread artifact types and removes duplicate note versions from the total', () => {
    expect(
      buildReviewQueueData({
        notes: [
          createNote(
            'fake-note-a',
            'fake-version-a',
            '重複するノート',
            '2026-10-11T01:00:00Z',
          ),
          createNote(
            'fake-note-b',
            'fake-version-b',
            '未読ノート',
            '2026-10-08T01:00:00Z',
          ),
        ],
        noteVersions: [
          createNoteVersion(
            'fake-version-a',
            'fake-note-a',
            '承認待ちバージョン',
            '2026-10-11T02:00:00Z',
          ),
          createNoteVersion(
            'fake-version-c',
            'fake-note-c',
            '別の承認待ちバージョン',
            '2026-10-09T01:00:00Z',
          ),
        ],
        annotations: [
          createAnnotation(
            'fake-annotation-a',
            'unread',
            '2026-10-10T01:00:00Z',
          ),
          createAnnotation(
            'fake-annotation-b',
            'approved',
            '2026-10-12T01:00:00Z',
          ),
        ],
      }),
    ).toEqual({
      counts: { notes: 2, noteVersions: 2, annotations: 1, total: 4 },
      items: [
        {
          kind: 'note-version',
          id: 'fake-version-a',
          noteId: 'fake-note-a',
          versionNo: 1,
          title: '承認待ちバージョン',
          createdAt: '2026-10-11T02:00:00Z',
        },
        {
          kind: 'annotation',
          id: 'fake-annotation-a',
          title: '価格帯を確認する',
          createdAt: '2026-10-10T01:00:00Z',
          targetSymbol: '0000',
        },
        {
          kind: 'note-version',
          id: 'fake-version-c',
          noteId: 'fake-note-c',
          versionNo: 1,
          title: '別の承認待ちバージョン',
          createdAt: '2026-10-09T01:00:00Z',
        },
      ],
    })
  })
})
