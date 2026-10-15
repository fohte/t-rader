import type { Meta, StoryObj } from '@storybook/react-vite'
import { RouterProvider } from '@tanstack/react-router'

import { ExecutionOriginLinks } from '#components/execution-origin-links'
import { createStoryRouter } from '#storybook/story-router'

const meta = {
  title: 'Common/ExecutionOriginLinks',
  component: ExecutionOriginLinks,
  args: {
    origin: {
      strategyId: 'bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb',
      strategyName: '架空戦略',
      taskId: 'cccccccc-cccc-4ccc-8ccc-cccccccccccc',
    },
  },
  decorators: [
    (Story) => (
      <RouterProvider
        router={createStoryRouter(
          () => (
            <Story />
          ),
          {
            paths: ['/strategies/$id', '/strategies/$id/runs/$taskId'],
            initialPath: '/strategies/sample-strategy',
          },
        )}
      />
    ),
  ],
} satisfies Meta<typeof ExecutionOriginLinks>

export default meta
type Story = StoryObj<typeof meta>

export const StrategyAndRunLinks: Story = {
  name: 'links to the strategy and the run that generated the content.',
}
