import type { Meta, StoryObj } from '@storybook/react-vite'
import { RouterProvider } from '@tanstack/react-router'

import { Header } from '#components/strategy-shell/header'
import { createStoryRouter } from '#storybook/story-router'
import { StrategySwitcherQueryDecorator } from '#storybook/strategy-switcher-mock'

function createHeaderRouter(initialPath: string) {
  return createStoryRouter(() => <Header />, {
    paths: [
      '/strategies',
      '/strategies/$id/performance',
      '/strategies/$id/settings',
      '/portfolio',
      '/trades',
      '/notes',
      '/annotations',
      '/hypotheses',
      '/runs',
      '/indicators',
    ],
    initialPath,
  })
}

const meta = {
  title: 'StrategyShell/Header',
  parameters: { layout: 'fullscreen' },
  decorators: [
    (Story) => (
      <StrategySwitcherQueryDecorator>
        <Story />
      </StrategySwitcherQueryDecorator>
    ),
  ],
} satisfies Meta

export default meta
type Story = StoryObj<typeof meta>

export const StrategyList: Story = {
  name: 'shows strategy navigation on the strategy list page.',
  render: () => <RouterProvider router={createHeaderRouter('/strategies')} />,
}

export const StrategyHome: Story = {
  name: 'shows the header on a strategy performance page.',
  render: () => (
    <RouterProvider
      router={createHeaderRouter('/strategies/semi-swing/performance')}
    />
  ),
}

export const Portfolio: Story = {
  name: 'shows the portfolio page with portfolio navigation active.',
  render: () => <RouterProvider router={createHeaderRouter('/portfolio')} />,
}
