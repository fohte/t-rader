import type { Meta, StoryObj } from '@storybook/react-vite'
import { RouterProvider } from '@tanstack/react-router'
import type { ComponentType } from 'react'

import { PendingNoteVersionsLink } from '#components/note-detail/pending-note-versions-link'
import { createStoryRouter } from '#storybook/story-router'

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
  title: 'NoteDetail/PendingNoteVersionsLink',
  component: PendingNoteVersionsLink,
  parameters: { layout: 'padded' },
  decorators: [
    (Story) => (
      <div className="bg-background text-foreground">{withRouter(Story)}</div>
    ),
  ],
} satisfies Meta<typeof PendingNoteVersionsLink>

export default meta
type Story = StoryObj<typeof meta>

export const Default: Story = {}
