import type { Meta, StoryObj } from '@storybook/react-vite'
import { Outlet, RouterProvider } from '@tanstack/react-router'

import { NotesTabs } from '#components/notes/notes-tabs'
import { createStoryRouter } from '#storybook/story-router'

const meta = {
  title: 'Notes/NotesTabs',
  component: NotesTabs,
  decorators: [
    (Story) => (
      <RouterProvider
        router={createStoryRouter(
          () => (
            <Outlet />
          ),
          {
            paths: [{ path: '/notes', component: () => <Story /> }],
            initialPath: '/notes',
          },
        )}
      />
    ),
  ],
  args: { activeTab: 'notes' },
} satisfies Meta<typeof NotesTabs>

export default meta
type Story = StoryObj<typeof meta>

export const NotesSelected: Story = {
  name: 'the notes tab is selected.',
}

export const AnnotationsSelected: Story = {
  name: 'the annotations tab is selected.',
  args: { activeTab: 'annotations' },
}
