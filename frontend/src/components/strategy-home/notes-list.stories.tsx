import type { Meta, StoryObj } from '@storybook/react-vite'
import { RouterProvider } from '@tanstack/react-router'

import { NotesList } from '#components/strategy-home/notes-list'
import type { components } from '#lib/api/schema.gen'
import { createStoryRouter } from '#storybook/story-router'

type Note = components['schemas']['Note']

const notes: Note[] = [
  {
    id: '00000000-0000-0000-0000-000000000001',
    version_id: '00000000-0000-0000-0000-000000000002',
    version_no: 1,
    is_current: true,
    title: '架空銘柄の検討メモ',
    body_md: '検証用の本文です。',
    frontmatter_json: {},
    graphs_json: [],
    kind: 'sample-kind',
    status: 'unread',
    trigger: null,
    trigger_label: null,
    created_by_kind: 'llm',
    created_at: '2026-01-01T00:00:00Z',
    updated_at: '2026-01-02T00:00:00Z',
  },
]

const meta = {
  title: 'StrategyHome/NotesList',
  component: NotesList,
  decorators: [
    (Story) => (
      <RouterProvider
        router={createStoryRouter(
          () => (
            <Story />
          ),
          { paths: ['/notes/$noteId'] },
        )}
      />
    ),
  ],
  args: { notes },
} satisfies Meta<typeof NotesList>

export default meta
type Story = StoryObj<typeof meta>

export const WithNotes: Story = {
  name: 'shows notes with their status and update time.',
}

export const Empty: Story = {
  name: 'shows the empty state when there are no notes.',
  args: { notes: [] },
}
