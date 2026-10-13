import type { Meta, StoryObj } from '@storybook/react-vite'
import { RouterProvider } from '@tanstack/react-router'
import type { ComponentType } from 'react'

import { NoteLinkRow } from '#components/notes/note-link-row'
import { createStoryRouter } from '#storybook/story-router'

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
  title: 'Notes/NoteLinkRow',
  component: NoteLinkRow,
  decorators: [withRouter],
  args: {
    noteId: 'fake-note-a',
    title: '架空企業の確認メモ',
    status: 'unread',
    strategyName: '架空戦略',
    author: 'エージェント',
    updatedAt: '2026-10-10T03:00:00Z',
  },
} satisfies Meta<typeof NoteLinkRow>

export default meta
type Story = StoryObj<typeof meta>

export const Unread: Story = {
  name: 'The row shows an unread pill and its strategy.',
}

export const Approved: Story = {
  name: 'The row omits the unread pill for an approved note.',
  args: { status: 'approved', author: '自分' },
}
