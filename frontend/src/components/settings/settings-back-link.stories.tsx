import type { Meta, StoryObj } from '@storybook/react-vite'
import { RouterProvider } from '@tanstack/react-router'

import { SettingsBackLink } from '#components/settings/settings-back-link'
import { createStoryRouter } from '#storybook/story-router'

const meta = {
  title: 'Settings/SettingsBackLink',
  component: SettingsBackLink,
  parameters: { layout: 'padded' },
  render: () => (
    <RouterProvider
      router={createStoryRouter(
        () => (
          <SettingsBackLink />
        ),
        {
          paths: ['/settings'],
        },
      )}
    />
  ),
} satisfies Meta<typeof SettingsBackLink>

export default meta
type Story = StoryObj<typeof meta>

export const Default: Story = {
  name: '設定画面へ戻るリンクを表示します。',
}
