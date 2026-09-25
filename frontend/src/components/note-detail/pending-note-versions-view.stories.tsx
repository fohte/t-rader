import type { Meta, StoryObj } from '@storybook/react-vite'
import { RouterProvider } from '@tanstack/react-router'
import type { ComponentType } from 'react'

import {
  PendingNoteVersionsView,
  type PendingNoteVersionsViewProps,
} from '#components/note-detail/pending-note-versions-view'
import type { NoteVersion } from '#lib/api/note-version-types'
import { createStoryRouter } from '#storybook/story-router'

const versions: NoteVersion[] = [
  {
    id: 'fake-version-a',
    note_id: 'fake-note-a',
    version_no: 4,
    title: '架空データの追加確認',
    body_md: '追加した本文です。',
    frontmatter_json: {},
    graphs_json: [],
    status: 'unread',
    is_current: false,
    change_reason: '追加資料の確認結果を追記しました。',
    created_by_kind: 'llm',
    execution_id: null,
    created_at: '2026-01-12T02:00:00Z',
    reviewed_at: null,
  },
  {
    id: 'fake-version-b',
    note_id: 'fake-note-b',
    version_no: 2,
    title: '架空の月次レビュー',
    body_md: '比較に使う仮の本文です。',
    frontmatter_json: {},
    graphs_json: [],
    status: 'unread',
    is_current: false,
    change_reason: null,
    created_by_kind: 'human',
    execution_id: null,
    created_at: '2026-01-11T02:00:00Z',
    reviewed_at: null,
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
          paths: ['/notes/$noteId'],
        },
      )}
    />
  )
}

const args: PendingNoteVersionsViewProps = {
  versions,
  isPending: false,
  hasError: false,
}

const meta = {
  title: 'NoteDetail/PendingNoteVersionsView',
  component: PendingNoteVersionsView,
  parameters: { layout: 'padded' },
  args,
  decorators: [
    (Story) => (
      <div className="max-w-3xl bg-background text-foreground">
        {withRouter(Story)}
      </div>
    ),
  ],
} satisfies Meta<typeof PendingNoteVersionsView>

export default meta
type Story = StoryObj<typeof meta>

export const MultipleNotes: Story = {
  name: 'shows pending versions from multiple notes.',
}

export const Loading: Story = {
  name: 'shows loading placeholders while pending versions load.',
  args: { versions: [], isPending: true },
}

export const Empty: Story = {
  name: 'shows the empty state when no versions await review.',
  args: { versions: [] },
}

export const Error: Story = {
  name: 'shows an error when pending versions fail to load.',
  args: { versions: [], hasError: true },
}
