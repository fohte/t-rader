import type { Meta, StoryObj } from '@storybook/react-vite'
import { RouterProvider } from '@tanstack/react-router'
import type { ComponentType } from 'react'

import { NoteVersionList } from '#components/note-detail/note-version-list'
import type { NoteVersion } from '#lib/api/note-version-types'
import { createStoryRouter } from '#storybook/story-router'

const versions: NoteVersion[] = [
  {
    id: 'fake-version-a',
    note_id: 'fake-note-a',
    version_no: 3,
    title: '架空データの追加確認',
    body_md: '変更箇所です。',
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
    note_id: 'fake-note-a',
    version_no: 2,
    title: '架空データの確認',
    body_md: '以前の本文です。',
    frontmatter_json: {},
    graphs_json: [],
    status: 'approved',
    is_current: true,
    change_reason: null,
    created_by_kind: 'human',
    execution_id: null,
    created_at: '2026-01-10T02:00:00Z',
    reviewed_at: '2026-01-10T03:00:00Z',
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
          paths: ['/note-versions/pending'],
        },
      )}
    />
  )
}

const meta = {
  title: 'NoteDetail/NoteVersionList',
  component: NoteVersionList,
  parameters: { layout: 'padded' },
  args: {
    versions,
    selectedVersionNo: 3,
    onSelectVersion: () => {},
  },
  decorators: [
    (Story) => (
      <div className="max-w-md bg-background text-foreground">
        {withRouter(Story)}
      </div>
    ),
  ],
} satisfies Meta<typeof NoteVersionList>

export default meta
type Story = StoryObj<typeof meta>

export const SelectedPending: Story = {}

export const SelectedCurrent: Story = {
  args: { selectedVersionNo: 2 },
}

export const Empty: Story = { args: { versions: [] } }
