import type { Meta, StoryObj } from '@storybook/react-vite'
import { RouterProvider } from '@tanstack/react-router'
import type { ComponentType } from 'react'

import {
  approvedNoteOverview,
  currentAndUnreadNoteOverview,
  pendingNoteOverview,
} from '#components/dashboard/dashboard-notes.fixtures'
import { RelatedNotes } from '#components/related-notes'
import { createStoryRouter } from '#storybook/story-router'

const notes = [
  currentAndUnreadNoteOverview,
  approvedNoteOverview,
  pendingNoteOverview,
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
  name: 'shows related notes with all review states.',
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
