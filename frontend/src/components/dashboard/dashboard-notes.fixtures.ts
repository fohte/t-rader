import type { components } from '#lib/api/schema.gen'

type Note = components['schemas']['Note']
type NoteVersion = components['schemas']['NoteVersion']
type NoteOverview = components['schemas']['NoteOverviewResponse']

function versionFromNote(note: Note): NoteVersion {
  return {
    id: note.version_id,
    note_id: note.id,
    version_no: note.version_no,
    is_current: note.is_current,
    title: note.title,
    body_md: note.body_md,
    frontmatter_json: note.frontmatter_json,
    resolved_price_references_json: note.resolved_price_references_json,
    graphs_json: note.graphs_json,
    warning_dispositions: [],
    status: note.status,
    created_by_kind: note.created_by_kind,
    created_at: note.created_at,
  }
}

const pendingNote: Note = {
  id: 'fake-note-pending',
  version_id: 'fake-version-pending',
  version_no: 1,
  is_current: false,
  title: '現行なしの架空ノート',
  body_md: '表示用の架空ノートです。',
  frontmatter_json: {},
  resolved_price_references_json: {},
  graphs_json: [],
  tags: [],
  kind: null,
  status: 'unread',
  trigger: null,
  trigger_label: null,
  created_by_kind: 'llm',
  created_at: '2026-10-10T03:00:00Z',
  updated_at: '2026-10-10T03:00:00Z',
}

const pendingVersion = versionFromNote(pendingNote)

export const pendingNoteOverview: NoteOverview = {
  representative: pendingNote,
  current: null,
  latest_unread: pendingVersion,
  unread_version_count: 1,
}

const approvedNote: Note = {
  id: 'fake-note-a',
  version_id: 'fake-version-a',
  version_no: 1,
  is_current: true,
  title: '架空企業の決算前メモ',
  body_md: '表示用の架空ノートです。',
  frontmatter_json: {},
  resolved_price_references_json: {},
  graphs_json: [],
  tags: [],
  kind: null,
  status: 'approved',
  trigger: null,
  trigger_label: null,
  created_by_kind: 'llm',
  created_at: '2026-10-10T03:00:00Z',
  updated_at: '2026-10-10T03:00:00Z',
}

export const approvedNoteOverview: NoteOverview = {
  representative: approvedNote,
  current: versionFromNote(approvedNote),
  latest_unread: null,
  unread_version_count: 0,
}

const humanApprovedNote: Note = {
  ...approvedNote,
  id: 'fake-note-b',
  version_id: 'fake-version-b',
  title: '売買後の振り返り',
  created_by_kind: 'human',
  created_at: '2026-10-09T03:00:00Z',
  updated_at: '2026-10-09T03:00:00Z',
}

export const humanApprovedNoteOverview: NoteOverview = {
  representative: humanApprovedNote,
  current: versionFromNote(humanApprovedNote),
  latest_unread: null,
  unread_version_count: 0,
}

const currentWithUnreadNote: Note = {
  ...approvedNote,
  id: 'fake-note-current-unread',
  version_id: 'fake-version-current',
  title: '現行バージョンと未読バージョンを持つ架空ノート',
  updated_at: '2026-10-11T03:00:00Z',
}

const unreadVersionForCurrentNote: Note = {
  ...currentWithUnreadNote,
  version_id: 'fake-version-unread',
  version_no: 2,
  is_current: false,
  status: 'unread',
  created_at: '2026-10-11T04:00:00Z',
  updated_at: '2026-10-11T04:00:00Z',
}

export const currentAndUnreadNoteOverview: NoteOverview = {
  representative: currentWithUnreadNote,
  current: versionFromNote(currentWithUnreadNote),
  latest_unread: versionFromNote(unreadVersionForCurrentNote),
  unread_version_count: 1,
}
