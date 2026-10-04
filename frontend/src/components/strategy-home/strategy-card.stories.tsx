import type { Meta, StoryObj } from '@storybook/react-vite'
import { RouterProvider } from '@tanstack/react-router'

import { StrategyCard } from '#components/strategy-home/strategy-card'
import type { components } from '#lib/api/schema.gen'
import { createStoryRouter } from '#storybook/story-router'

type Strategy = components['schemas']['Strategy']

const strategy: Strategy = {
  id: '00000000-0000-0000-0000-000000000001',
  name: '架空戦略',
  description: 'サンプルとして表示する説明文です。',
  sort_order: 0,
  created_at: '2026-01-01T00:00:00Z',
  updated_at: '2026-01-02T00:00:00Z',
}

const meta = {
  title: 'StrategyHome/StrategyCard',
  component: StrategyCard,
  decorators: [
    (Story) => (
      <RouterProvider
        router={createStoryRouter(
          () => (
            <Story />
          ),
          { paths: ['/strategies/$id'] },
        )}
      />
    ),
  ],
  args: { strategy },
} satisfies Meta<typeof StrategyCard>

export default meta
type Story = StoryObj<typeof meta>

export const WithDescription: Story = {
  name: 'shows a strategy card with its description and updated time.',
}

export const WithoutDescription: Story = {
  name: 'shows a strategy card without a description.',
  args: { strategy: { ...strategy, description: null } },
}
