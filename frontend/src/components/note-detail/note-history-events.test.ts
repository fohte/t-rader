import { describe, expect, it } from 'vitest'

import { groupNoteHistoryEvents } from '#components/note-detail/note-history-events'
import type { NoteVersion } from '#lib/api/note-version-types'
import type { components } from '#lib/api/schema.gen'

type ChangeHistory = components['schemas']['ChangeHistory']

function makeVersion(
  id: string,
  versionNo: number,
  createdAt: string,
): NoteVersion {
  return {
    id,
    note_id: 'fixture-note',
    version_no: versionNo,
    title: '架空ノート',
    body_md: '架空の本文です。',
    frontmatter_json: {},
    resolved_price_references_json: {},
    warning_dispositions: [],
    graphs_json: [],
    status: 'approved',
    is_current: versionNo === 2,
    change_reason: null,
    created_by_kind: 'human',
    execution_id: null,
    created_at: createdAt,
    reviewed_at: null,
  }
}

function makeHistory(
  id: string,
  op: string,
  createdAt: string,
  diff: ChangeHistory['diff_json'],
): ChangeHistory {
  return {
    id,
    target_kind: 'note',
    target_id: 'fixture-note',
    actor_kind: 'human',
    actor_label: '自分',
    op,
    diff_json: diff,
    summary: null,
    created_at: createdAt,
  }
}

function summarizeEvents(grouped: ReturnType<typeof groupNoteHistoryEvents>) {
  return {
    orderedVersions: grouped.orderedVersions.map((version) => version.id),
    eventsByVersionId: Object.fromEntries(
      grouped.orderedVersions.map((version) => [
        version.id,
        (grouped.eventsByVersionId.get(version.id) ?? []).map(
          ({ entry, label }) => ({ id: entry.id, label }),
        ),
      ]),
    ),
    unmatchedEvents: grouped.unmatchedEvents.map(({ entry, label }) => ({
      id: entry.id,
      label,
    })),
  }
}

describe('groupNoteHistoryEvents', () => {
  it('groups unversioned events by the latest version created at their timestamps', () => {
    const versions = [
      makeVersion('fixture-version-three', 3, '2026-10-13T09:00:00Z'),
      makeVersion('fixture-version-two', 2, '2026-10-10T13:00:00Z'),
    ]
    const history = [
      makeHistory(
        'fixture-history-after-version-three',
        'update',
        '2026-10-13T10:00:00Z',
        { title: { from: '確認前', to: '確認後' } },
      ),
      makeHistory(
        'fixture-history-before-version-three',
        'update',
        '2026-10-13T08:59:00Z',
        { title: { from: '以前', to: '更新' } },
      ),
      makeHistory(
        'fixture-history-at-version-three',
        'update',
        '2026-10-13T09:00:00Z',
        { title: { from: '更新前', to: '更新後' } },
      ),
      makeHistory(
        'fixture-history-current-version-change',
        'update',
        '2026-10-12T12:00:00Z',
        {
          from_version_id: 'fixture-version-three',
          to_version_id: 'fixture-version-two',
        },
      ),
      makeHistory(
        'fixture-history-approval',
        'status_change',
        '2026-10-11T12:00:00Z',
        {
          from: 'unread',
          to: 'approved',
          version_id: 'fixture-version-two',
        },
      ),
      makeHistory(
        'fixture-history-before-first-version',
        'delete',
        '2026-10-09T12:00:00Z',
        {},
      ),
    ]

    const grouped = groupNoteHistoryEvents(versions, history)
    expect(summarizeEvents(grouped)).toEqual({
      orderedVersions: ['fixture-version-three', 'fixture-version-two'],
      eventsByVersionId: {
        'fixture-version-three': [
          { id: 'fixture-history-after-version-three', label: '更新' },
          { id: 'fixture-history-at-version-three', label: '更新' },
        ],
        'fixture-version-two': [
          { id: 'fixture-history-before-version-three', label: '更新' },
          { id: 'fixture-history-current-version-change', label: '現行に変更' },
          { id: 'fixture-history-approval', label: '承認' },
        ],
      },
      unmatchedEvents: [
        { id: 'fixture-history-before-first-version', label: '削除' },
      ],
    })
  })
})
