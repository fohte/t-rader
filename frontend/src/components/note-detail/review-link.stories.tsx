import type { Meta, StoryObj } from '@storybook/react-vite'
import { RouterProvider } from '@tanstack/react-router'
import type { ComponentType } from 'react'

import { ReviewLink } from '#components/note-detail/review-link'
import { createStoryRouter } from '#storybook/story-router'

function withRouter(Story: ComponentType) {
  return (
    <RouterProvider
      router={createStoryRouter(
        () => (
          <Story />
        ),
        {
          paths: ['/notes'],
        },
      )}
    />
  )
}

const meta = {
  title: 'NoteDetail/ReviewLink',
  component: ReviewLink,
  parameters: { layout: 'padded' },
  decorators: [
    (Story) => (
      <div className="bg-background text-foreground">{withRouter(Story)}</div>
    ),
  ],
} satisfies Meta<typeof ReviewLink>

export default meta
type Story = StoryObj<typeof meta>

export const Default: Story = {
  name: 'opens the unified unread list from a note.',
}
