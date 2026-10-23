import type { Meta, StoryObj } from '@storybook/react-vite'
import { RouterProvider } from '@tanstack/react-router'
import type { ComponentType } from 'react'

import { NotesList } from '#components/strategy-home/notes-list'
import type { components } from '#lib/api/schema.gen'
import { createStoryRouter } from '#storybook/story-router'

type Note = components['schemas']['Note']
type NoteKind = components['schemas']['NoteKind']

const note: Note = {
  id: 'fake-note-a',
  version_id: 'fake-version-a',
  version_no: 1,
  is_current: true,
  title: '架空ノートの確認',
  body_md: '表示確認用の本文です。',
  frontmatter_json: {},
  resolved_price_references_json: {},
  graphs_json: [],
  tags: ['架空タグ'],
  kind: 'sample-kind',
  status: 'unread',
  trigger: null,
  trigger_label: null,
  created_by_kind: 'human',
  created_at: '2026-01-01T00:00:00Z',
  updated_at: '2026-01-01T00:00:00Z',
}

const notes: Note[] = [note]

const noteKinds: NoteKind[] = [
  {
    key: 'sample-kind',
    display_name: '架空の種別表示名',
    requires_approval: false,
    sort_order: 0,
  },
  {
    key: 'sample-long-kind',
    display_name: '長い架空の種別表示名を折り返して読める状態',
    requires_approval: false,
    sort_order: 1,
  },
]

function withRouter(Story: ComponentType) {
  return (
    <RouterProvider
      router={createStoryRouter(
        () => (
          <Story />
        ),
        { paths: ['/notes/$noteId'] },
      )}
    />
  )
}

const meta = {
  title: 'StrategyHome/NotesList',
  component: NotesList,
  args: { notes, noteKinds, noteKindCatalogState: 'loaded' },
  decorators: [withRouter],
} satisfies Meta<typeof NotesList>

export default meta
type Story = StoryObj<typeof meta>

export const WithTags: Story = {
  name: 'shows a note with its kind, tags, and review status.',
}

export const CatalogLoading: Story = {
  name: 'shows that kind labels are loading without exposing their keys.',
  args: { noteKindCatalogState: 'loading' },
}

export const CatalogFailed: Story = {
  name: 'shows that kind labels could not be loaded.',
  args: { noteKindCatalogState: 'error' },
}

export const KindUnregistered: Story = {
  name: 'shows when a note kind is absent from the loaded catalog.',
  args: {
    notes: [{ ...note, kind: 'unlisted-kind' }],
    noteKinds: [],
  },
}

export const KindUnset: Story = {
  name: 'shows when a note has no kind.',
  args: { notes: [{ ...note, kind: null }] },
}

export const LongKind: Story = {
  name: 'shows a long kind label alongside the note title.',
  args: {
    notes: [{ ...note, kind: 'sample-long-kind' }],
  },
}

export const Empty: Story = {
  name: 'The list shows an empty state when no notes are available.',
  args: { notes: [] },
}
