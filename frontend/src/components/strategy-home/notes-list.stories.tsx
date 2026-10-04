import type { Meta, StoryObj } from '@storybook/react-vite'
import { RouterProvider } from '@tanstack/react-router'
import type { ComponentType } from 'react'

import { NotesList } from '#components/strategy-home/notes-list'
import type { components } from '#lib/api/schema.gen'
import { createStoryRouter } from '#storybook/story-router'

type Note = components['schemas']['Note']

const notes: Note[] = [
  {
    id: 'fake-note-a',
    version_id: 'fake-version-a',
    version_no: 1,
    is_current: true,
    title: '架空ノートの確認',
    body_md: '表示確認用の本文です。',
    frontmatter_json: {},
    graphs_json: [],
    tags: ['架空タグ'],
    kind: null,
    status: 'unread',
    trigger: null,
    trigger_label: null,
    created_by_kind: 'human',
    created_at: '2026-01-01T00:00:00Z',
    updated_at: '2026-01-01T00:00:00Z',
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
  args: { notes },
  decorators: [withRouter],
} satisfies Meta<typeof NotesList>

export default meta
type Story = StoryObj<typeof meta>

export const WithTags: Story = {
  name: 'The list shows tags for each note.',
}

export const Empty: Story = {
  name: 'The list shows an empty state when no notes are available.',
  args: { notes: [] },
}
