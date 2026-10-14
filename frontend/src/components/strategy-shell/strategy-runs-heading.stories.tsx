import type { Meta, StoryObj } from '@storybook/react-vite'
import { RouterProvider } from '@tanstack/react-router'

import { StrategyRunsHeading } from '#components/strategy-shell/strategy-runs-heading'
import { createStoryRouter } from '#storybook/story-router'

const meta = {
  title: 'StrategyShell/StrategyRunsHeading',
  component: StrategyRunsHeading,
  parameters: { layout: 'padded' },
  render: () => (
    <RouterProvider
      router={createStoryRouter(
        () => (
          <StrategyRunsHeading />
        ),
        {
          paths: ['/strategies'],
          initialPath: '/strategies',
        },
      )}
    />
  ),
} satisfies Meta<typeof StrategyRunsHeading>

export default meta
type Story = StoryObj<typeof meta>

export const StrategyRuns: Story = {
  name: 'the heading links to run history across all strategies.',
}
