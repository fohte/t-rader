import type { Meta, StoryObj } from '@storybook/react-vite'
import { RouterProvider } from '@tanstack/react-router'
import type { ComponentType } from 'react'

import { NoteScreenerLink } from '#components/note-detail/note-screener-link'
import { createStoryRouter } from '#storybook/story-router'

function withRouter(Story: ComponentType) {
  return (
    <RouterProvider
      router={createStoryRouter(
        () => (
          <Story />
        ),
        { paths: ['/screener'] },
      )}
    />
  )
}

const meta = {
  title: 'NoteDetail/NoteScreenerLink',
  component: NoteScreenerLink,
  parameters: { layout: 'padded' },
  decorators: [
    (Story) => (
      <div className="bg-background text-foreground">{withRouter(Story)}</div>
    ),
  ],
} satisfies Meta<typeof NoteScreenerLink>

export default meta
type Story = StoryObj<typeof meta>

export const Available: Story = {
  name: 'links to the screener when the note has a saved screen id.',
  args: { screenId: 'fake-screen-a' },
}
