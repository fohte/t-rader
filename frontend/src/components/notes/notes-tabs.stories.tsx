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
  args: {
    activeTab: 'notes',
    annotationUnreadCount: 2,
    annotationUnreadCountState: 'loaded',
  },
} satisfies Meta<typeof NotesTabs>

export default meta
type Story = StoryObj<typeof meta>

export const NotesSelected: Story = {
  name: 'the notes tab is selected.',
}

export const AnnotationsSelected: Story = {
  name: 'the annotations tab is selected.',
  args: {
    activeTab: 'annotations',
    annotationUnreadCount: 2,
    annotationUnreadCountState: 'loaded',
  },
}

export const NoUnreadAnnotations: Story = {
  name: 'the annotations tab has no unread items.',
  args: {
    activeTab: 'notes',
    annotationUnreadCount: 0,
    annotationUnreadCountState: 'loaded',
  },
}

export const LoadingUnreadCount: Story = {
  name: 'the unread annotation count is loading.',
  args: {
    activeTab: 'notes',
    annotationUnreadCountState: 'loading',
  },
}

export const UnavailableUnreadCount: Story = {
  name: 'the unread annotation count could not be loaded.',
  args: {
    activeTab: 'notes',
    annotationUnreadCountState: 'error',
  },
}
