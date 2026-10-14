import type { Meta, StoryObj } from '@storybook/react-vite'
import { RouterProvider } from '@tanstack/react-router'

import { SettingsIndexItems } from '#components/settings/settings-index-items'
import { createStoryRouter } from '#storybook/story-router'

const meta = {
  title: 'Settings/SettingsIndexItems',
  component: SettingsIndexItems,
  parameters: { layout: 'padded' },
} satisfies Meta<typeof SettingsIndexItems>

export default meta
type Story = StoryObj<typeof meta>

export const Default: Story = {
  name: 'the settings list includes shared custom indicators.',
  render: () => (
    <RouterProvider
      router={createStoryRouter(
        () => (
          <SettingsIndexItems />
        ),
        {
          paths: [
            '/settings',
            '/settings/agent-configs',
            '/settings/rss-feeds',
            '/settings/ingest-status',
            '/settings/risk-policy',
            '/settings/indicators',
          ],
          initialPath: '/settings',
        },
      )}
    />
  ),
}
