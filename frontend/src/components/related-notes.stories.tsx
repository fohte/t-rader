import type { Meta, StoryObj } from '@storybook/react-vite'
import { RouterProvider } from '@tanstack/react-router'
import type { ComponentType } from 'react'

import { RelatedNotes } from '#components/related-notes'
import type { components } from '#lib/api/schema.gen'
import { createStoryRouter } from '#storybook/story-router'

type Note = components['schemas']['Note']

const notes: Note[] = [
  {
    id: 'sample-note-coral-02',
    version_id: 'sample-version-coral-02',
    version_no: 1,
    is_current: true,
    title: '季節ごとの変化を記録するメモ',
    body_md: '画面表示用の架空ノートです。',
    frontmatter_json: {},
    resolved_price_references_json: {},
    graphs_json: [],
    tags: [],
    kind: null,
    status: 'unread',
    trigger: null,
    trigger_label: null,
    created_by_kind: 'llm',
    created_at: '2020-02-03T00:00:00Z',
    updated_at: '2020-02-03T00:00:00Z',
  },
  {
    id: 'sample-note-coral-01',
    version_id: 'sample-version-coral-01',
    version_no: 1,
    is_current: true,
    title: '定期確認の記録',
    body_md: '画面表示用の架空ノートです。',
    frontmatter_json: {},
    resolved_price_references_json: {},
    graphs_json: [],
    tags: [],
    kind: null,
    status: 'approved',
    trigger: null,
    trigger_label: null,
    created_by_kind: 'human',
    created_at: '2020-02-01T00:00:00Z',
    updated_at: '2020-02-01T00:00:00Z',
  },
]

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
  title: 'Components/RelatedNotes',
  component: RelatedNotes,
  decorators: [withRouter],
  args: { notes, isError: false },
} satisfies Meta<typeof RelatedNotes>

export default meta
type Story = StoryObj<typeof meta>

export const Populated: Story = {
  name: 'shows related notes from newest to oldest.',
}

export const Empty: Story = {
  name: 'shows an empty state when no related notes exist.',
  args: { notes: [] },
}

export const Loading: Story = {
  name: 'shows placeholders while related notes are loading.',
  args: { notes: null },
}

export const Error: Story = {
  name: 'shows an error message when related notes cannot be loaded.',
  args: { notes: null, isError: true },
}
