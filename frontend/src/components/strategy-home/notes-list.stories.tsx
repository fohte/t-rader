import type { Meta, StoryObj } from '@storybook/react-vite'
import { RouterProvider } from '@tanstack/react-router'
import type { ComponentType } from 'react'

import type { ReviewQueueItem } from '#components/review/review-queue-model'
import { NotesList } from '#components/strategy-home/notes-list'
import {
  type NoteOverview,
  type NoteOverviewView,
} from '#components/strategy-home/notes-overview-model'
import type { components } from '#lib/api/schema.gen'
import { createStoryRouter } from '#storybook/story-router'

type Note = components['schemas']['Note']
type NoteVersion = components['schemas']['NoteVersion']
type NoteKind = components['schemas']['NoteKind']

function makeNote(overrides: Partial<Note> = {}): Note {
  return {
    id: 'fake-note-a',
    version_id: 'fake-version-a',
    version_no: 4,
    is_current: true,
    title: '架空ノートの確認',
    body_md: '表示確認用の本文です。',
    frontmatter_json: {},
    resolved_price_references_json: {},
    graphs_json: [],
    tags: ['架空タグ'],
    kind: 'sample-kind',
    status: 'approved',
    trigger: null,
    trigger_label: null,
    created_by_kind: 'human',
    created_at: '2026-01-01T00:00:00Z',
    updated_at: '2026-01-01T00:00:00Z',
    ...overrides,
  }
}

function makeVersion(overrides: Partial<NoteVersion> = {}): NoteVersion {
  return {
    id: 'fake-version-a',
    note_id: 'fake-note-a',
    version_no: 4,
    is_current: true,
    title: '架空ノートの確認',
    body_md: '表示確認用の本文です。',
    frontmatter_json: {},
    resolved_price_references_json: {},
    graphs_json: [],
    status: 'approved',
    created_by_kind: 'human',
    created_at: '2026-01-01T00:00:00Z',
    ...overrides,
  }
}

function makeOverview(
  noteOverrides: Partial<Note> = {},
  options: {
    current?: NoteVersion | null
    latestUnread?: NoteVersion | null
    unreadVersionCount?: number
  } = {},
): NoteOverview {
  const representative = makeNote(noteOverrides)
  return {
    representative,
    current:
      options.current === undefined
        ? makeVersion({
            id: representative.version_id,
            note_id: representative.id,
            version_no: representative.version_no,
            title: representative.title,
            status: representative.status,
            is_current: representative.is_current,
          })
        : options.current,
    latest_unread: options.latestUnread ?? null,
    unread_version_count: options.unreadVersionCount ?? 0,
  }
}

const noteKinds: NoteKind[] = [
  {
    key: 'sample-kind',
    display_name: '架空の種別表示名',
    requires_approval: true,
    sort_order: 0,
  },
  {
    key: 'sample-long-kind',
    display_name: '長い架空の種別表示名を折り返して読める状態',
    requires_approval: false,
    sort_order: 1,
  },
]

const allNotes: NoteOverview[] = [
  makeOverview(
    { id: 'fake-note-a', status: 'unread', title: '新しい架空ノート' },
    {
      current: makeVersion({
        id: 'fake-version-a',
        note_id: 'fake-note-a',
        version_no: 1,
        status: 'unread',
        is_current: true,
        created_by_kind: 'llm',
      }),
      latestUnread: makeVersion({
        id: 'fake-version-a',
        note_id: 'fake-note-a',
        version_no: 1,
        status: 'unread',
        is_current: true,
        created_by_kind: 'llm',
      }),
      unreadVersionCount: 1,
    },
  ),
  makeOverview(
    {
      id: 'fake-note-b',
      version_id: 'fake-version-b',
      title: '更新がある架空ノート',
    },
    {
      current: makeVersion({
        id: 'fake-version-b',
        note_id: 'fake-note-b',
        version_no: 3,
        status: 'approved',
        is_current: true,
      }),
      latestUnread: makeVersion({
        id: 'fake-version-b-unread',
        note_id: 'fake-note-b',
        version_no: 5,
        status: 'unread',
        is_current: false,
        created_by_kind: 'llm',
      }),
      unreadVersionCount: 2,
    },
  ),
  makeOverview(
    {
      id: 'fake-note-c',
      version_id: 'fake-version-c',
      version_no: 2,
      is_current: false,
      title: '現行がない架空ノート',
      status: 'rejected',
      kind: null,
      tags: [],
    },
    { current: null },
  ),
]

const firstOverview = allNotes[0] ?? makeOverview()

const selectedItem: ReviewQueueItem = {
  kind: 'note',
  id: 'fake-version-b-old',
  noteId: 'fake-note-b',
  versionNo: 4,
  title: '更新がある架空ノート',
  createdAt: '2026-01-01T00:00:00Z',
  noteKind: 'sample-kind',
  createdByKind: 'llm',
  unreadVersionCount: 2,
}

function withRouter(Story: ComponentType) {
  return (
    <RouterProvider
      router={createStoryRouter(
        () => (
          <Story />
        ),
        { paths: ['/notes', '/notes/$noteId'] },
      )}
    />
  )
}

const meta = {
  title: 'StrategyHome/NotesList',
  component: NotesList,
  args: {
    overviews: allNotes,
    view: 'all' as NoteOverviewView,
    state: 'loaded' as const,
    noteKinds,
    noteKindCatalogState: 'loaded' as const,
    selectedItemIds: [],
    selectedItems: [],
    isApproving: false,
    hasApprovalError: false,
    onToggleItem: () => {},
    onApproveSelected: () => {},
  },
  decorators: [withRouter],
} satisfies Meta<typeof NotesList>

export default meta
type Story = StoryObj<typeof meta>

export const Overview: Story = {
  name: 'shows current, unread, and rejected note versions in one row each.',
}

export const Pending: Story = {
  name: 'shows only notes with unread versions and their review counts.',
  args: { overviews: allNotes.slice(0, 2), view: 'pending' },
}

export const SelectedForApproval: Story = {
  name: 'keeps the selected version visible when a newer unread version arrives.',
  args: {
    overviews: allNotes,
    selectedItemIds: ['note:fake-note-b'],
    selectedItems: [selectedItem],
  },
}

export const ApprovalFailed: Story = {
  name: 'keeps failed and later selections after an approval error.',
  args: {
    selectedItemIds: ['note:fake-note-b'],
    selectedItems: [selectedItem],
    hasApprovalError: true,
  },
}

export const Loading: Story = {
  name: 'shows loading placeholders while note overviews are fetched.',
  args: { overviews: [], state: 'loading' },
}

export const Failed: Story = {
  name: 'shows an error when the overview request fails.',
  args: { overviews: [], state: 'error' },
}

export const Empty: Story = {
  name: 'shows the empty state when the selected view has no notes.',
  args: { overviews: [] },
}

export const PendingEmpty: Story = {
  name: 'shows the empty state when no unread notes are available.',
  args: { overviews: [], view: 'pending' },
}

export const CatalogLoading: Story = {
  name: 'shows that kind labels are loading without exposing their keys.',
  args: { overviews: [firstOverview], noteKindCatalogState: 'loading' },
}

export const CatalogFailed: Story = {
  name: 'shows that kind labels could not be loaded.',
  args: { overviews: [firstOverview], noteKindCatalogState: 'error' },
}

export const KindUnregistered: Story = {
  name: 'shows when a note kind is absent from the loaded catalog.',
  args: {
    overviews: [makeOverview({ kind: 'unlisted-kind', tags: [] })],
    noteKinds: [],
  },
}

export const KindUnset: Story = {
  name: 'shows when a note has no kind.',
  args: { overviews: [makeOverview({ kind: null })] },
}

export const LongKind: Story = {
  name: 'wraps a long kind label inside a narrow note row.',
  args: {
    overviews: [makeOverview({ kind: 'sample-long-kind' })],
  },
}

export const MobileSelection: Story = {
  name: 'keeps the selected approval action reachable on a phone screen.',
  globals: { viewport: { value: 'mobileReview' } },
  parameters: {
    viewport: {
      options: {
        mobileReview: {
          name: 'Mobile notes',
          styles: { width: '390px', height: '700px' },
          type: 'mobile',
        },
      },
    },
    screenshot: { viewport: { width: 390, height: 700 } },
  },
  args: {
    selectedItemIds: ['note:fake-note-b'],
    selectedItems: [selectedItem],
  },
}
