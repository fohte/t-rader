import type { Meta, StoryObj } from '@storybook/react-vite'
import { RouterProvider } from '@tanstack/react-router'
import type { ComponentType } from 'react'

import { DashboardRecentNotes } from '#components/dashboard/dashboard-recent-notes'
import type { components } from '#lib/api/schema.gen'
import { createStoryRouter } from '#storybook/story-router'

type Note = components['schemas']['Note']
type NoteVersion = components['schemas']['NoteVersion']
type NoteOverview = components['schemas']['NoteOverviewResponse']

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

const approvedVersion: NoteVersion = {
  id: approvedNote.version_id,
  note_id: approvedNote.id,
  version_no: approvedNote.version_no,
  is_current: true,
  title: approvedNote.title,
  body_md: approvedNote.body_md,
  frontmatter_json: approvedNote.frontmatter_json,
  resolved_price_references_json: approvedNote.resolved_price_references_json,
  graphs_json: approvedNote.graphs_json,
  warning_dispositions: [],
  status: approvedNote.status,
  created_by_kind: approvedNote.created_by_kind,
  created_at: approvedNote.created_at,
}

const pendingNote: Note = {
  id: 'fake-note-c',
  version_id: 'fake-version-c',
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
  created_at: '2026-10-11T03:00:00Z',
  updated_at: '2026-10-11T03:00:00Z',
}

const pendingVersion: NoteVersion = {
  id: pendingNote.version_id,
  note_id: pendingNote.id,
  version_no: pendingNote.version_no,
  is_current: false,
  title: pendingNote.title,
  body_md: pendingNote.body_md,
  frontmatter_json: pendingNote.frontmatter_json,
  resolved_price_references_json: pendingNote.resolved_price_references_json,
  graphs_json: pendingNote.graphs_json,
  warning_dispositions: [],
  status: pendingNote.status,
  created_by_kind: pendingNote.created_by_kind,
  created_at: pendingNote.created_at,
}

const notes: NoteOverview[] = [
  {
    representative: pendingNote,
    current: null,
    latest_unread: pendingVersion,
    unread_version_count: 1,
  },
  {
    representative: {
      id: 'fake-note-b',
      version_id: 'fake-version-b',
      version_no: 1,
      is_current: true,
      title: '売買後の振り返り',
      body_md: '表示用の架空ノートです。',
      frontmatter_json: {},
      resolved_price_references_json: {},
      graphs_json: [],
      tags: [],
      kind: null,
      status: 'approved',
      trigger: null,
      trigger_label: null,
      created_by_kind: 'human',
      created_at: '2026-10-09T03:00:00Z',
      updated_at: '2026-10-09T03:00:00Z',
    },
    current: {
      ...approvedVersion,
      id: 'fake-version-b',
      note_id: 'fake-note-b',
      title: '売買後の振り返り',
      created_by_kind: 'human',
      created_at: '2026-10-09T03:00:00Z',
    },
    latest_unread: null,
    unread_version_count: 0,
  },
]

function withRouter(Story: ComponentType) {
  return (
    <RouterProvider
      router={createStoryRouter(
        () => (
          <Story />
        ),
        {
          paths: ['/notes', '/notes/$noteId'],
        },
      )}
    />
  )
}

const meta = {
  title: 'Dashboard/DashboardRecentNotes',
  component: DashboardRecentNotes,
  decorators: [withRouter],
  args: { notes, isError: false },
} satisfies Meta<typeof DashboardRecentNotes>

export default meta
type Story = StoryObj<typeof meta>

export const Recent: Story = {
  name: 'shows recent notes from the user and an agent.',
}

export const PendingOnly: Story = {
  name: 'shows a pending note without a current version.',
  args: { notes: notes.slice(0, 1) },
}

export const Empty: Story = {
  name: 'shows an empty state when there are no notes.',
  args: { notes: [] },
}

export const Loading: Story = {
  name: 'shows placeholders while notes are loading.',
  args: { notes: null },
}

export const Error: Story = {
  name: 'shows an error message when notes cannot be loaded.',
  args: { notes: null, isError: true },
}
