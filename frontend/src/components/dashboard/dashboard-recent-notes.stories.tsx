import type { Meta, StoryObj } from '@storybook/react-vite'
import { RouterProvider } from '@tanstack/react-router'
import type { ComponentType } from 'react'

import {
  approvedNoteOverview,
  currentAndUnreadNoteOverview,
  humanApprovedNoteOverview,
  pendingNoteOverview,
} from '#components/dashboard/dashboard-notes.fixtures'
import { DashboardRecentNotes } from '#components/dashboard/dashboard-recent-notes'
import { createStoryRouter } from '#storybook/story-router'

const notes = [approvedNoteOverview, humanApprovedNoteOverview]

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
  args: { notes, isError: false },
} satisfies Meta<typeof DashboardRecentNotes>

export default meta
type Story = StoryObj<typeof meta>

export const Recent: Story = {
  name: 'shows recent notes from the user and the agent.',
}

export const PendingOnly: Story = {
  name: 'shows a pending note without a current version.',
  args: { notes: [pendingNoteOverview] },
}

export const CurrentAndUnread: Story = {
  name: 'shows an unread version alongside its current version.',
  args: { notes: [currentAndUnreadNoteOverview] },
}

export const Empty: Story = {
  name: 'shows an empty state when there are no notes.',
  args: { notes: [] },
}

export const Loading: Story = {
  name: 'shows placeholders while notes are loading.',
  args: { notes: null },
}

export const Error: Story = {
  name: 'shows an error message when notes cannot be loaded.',
  args: { notes: null, isError: true },
}
