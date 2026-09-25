import type { Meta, StoryObj } from '@storybook/react-vite'
import { RouterProvider } from '@tanstack/react-router'

import { HistoricalVersionNotice } from '#components/note-detail/historical-version-notice'
import { createStoryRouter } from '#storybook/story-router'

const meta = {
  title: 'NoteDetail/HistoricalVersionNotice',
  component: HistoricalVersionNotice,
  parameters: { layout: 'padded' },
  args: {
    noteId: '00000000-0000-0000-0000-000000000401',
    versionNo: 2,
  },
  decorators: [
    (Story) => (
      <RouterProvider
        router={createStoryRouter(
          () => (
            <div className="max-w-md bg-background p-5 text-foreground">
              <Story />
            </div>
          ),
          { paths: ['/notes/$noteId'] },
        )}
      />
    ),
  ],
} satisfies Meta<typeof HistoricalVersionNotice>

export default meta
type Story = StoryObj<typeof meta>

export const Historical: Story = {
  name: 'shows a notice that the displayed note version is historical.',
}
