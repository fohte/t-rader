import type { Meta, StoryObj } from '@storybook/react-vite'
import { RouterProvider } from '@tanstack/react-router'

import { IngestStatusPageHeader } from '#components/settings/ingest-status-page-header'
import { createStoryRouter } from '#storybook/story-router'

const meta = {
  title: 'Settings/IngestStatusPageHeader',
  component: IngestStatusPageHeader,
  parameters: { layout: 'padded' },
} satisfies Meta<typeof IngestStatusPageHeader>

export default meta
type Story = StoryObj<typeof meta>

export const Default: Story = {
  name: 'shows the page title and a link back to settings.',
  render: () => (
    <RouterProvider
      router={createStoryRouter(
        () => (
          <IngestStatusPageHeader />
        ),
        {
          paths: ['/settings'],
        },
      )}
    />
  ),
}
