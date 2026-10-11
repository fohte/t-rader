import type { Meta, StoryObj } from '@storybook/react-vite'
import { RouterProvider } from '@tanstack/react-router'
import type { ComponentType } from 'react'

import { DashboardRecentNotes } from '#components/dashboard/dashboard-recent-notes'
import type { components } from '#lib/api/schema.gen'
import { createStoryRouter } from '#storybook/story-router'

type Note = components['schemas']['Note']

const notes: Note[] = [
  {
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
  },
  {
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
  args: { notes },
} satisfies Meta<typeof DashboardRecentNotes>

export default meta
type Story = StoryObj<typeof meta>

export const Recent: Story = {
  name: 'shows recent notes from the user and the agent.',
}

export const Empty: Story = {
  name: 'shows an empty state when there are no notes.',
  args: { notes: [] },
}

export const Loading: Story = {
  name: 'shows placeholders while notes are loading.',
  args: { notes: null },
}
