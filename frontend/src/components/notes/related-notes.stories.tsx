import type { Meta, StoryObj } from '@storybook/react-vite'
import { RouterProvider } from '@tanstack/react-router'
import type { ComponentProps, ComponentType } from 'react'

import { RelatedNotes } from '#components/notes/related-notes'
import { createStoryRouter } from '#storybook/story-router'

type RelatedNote = ComponentProps<typeof RelatedNotes>['notes'][number]

const notes: RelatedNote[] = [
  {
    id: 'fake-note-a',
    title: '架空企業の業績確認',
    status: 'unread',
    strategyName: '架空戦略',
    author: 'エージェント',
    updatedAt: '2026-10-10T03:00:00Z',
  },
  {
    id: 'fake-note-b',
    title: '売買後の振り返り',
    status: 'approved',
    strategyName: '別の架空戦略',
    author: '自分',
    updatedAt: '2026-10-09T03:00:00Z',
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
  title: 'Notes/RelatedNotes',
  component: RelatedNotes,
  decorators: [withRouter],
  args: { notes },
} satisfies Meta<typeof RelatedNotes>

export default meta
type Story = StoryObj<typeof meta>

export const WithNotes: Story = {
  name: 'The list shows the newest related notes first.',
}

export const Empty: Story = {
  name: 'The list shows an empty state when no notes are related.',
  args: { notes: [] },
}
