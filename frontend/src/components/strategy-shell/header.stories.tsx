import type { Meta, StoryObj } from '@storybook/react-vite'
import { RouterProvider } from '@tanstack/react-router'

import { Header } from '#components/strategy-shell/header'
import { createStoryRouter } from '#storybook/story-router'

function createHeaderRouter(initialPath: string) {
  return createStoryRouter(() => <Header />, {
    paths: [
      '/strategies',
      '/strategies/$id',
      '/strategies/$id/settings',
      '/strategies/$id/runs',
      '/portfolio',
      '/trades',
    ],
    initialPath,
  })
}

const meta = {
  title: 'StrategyShell/Header',
  parameters: { layout: 'fullscreen' },
} satisfies Meta

export default meta
type Story = StoryObj<typeof meta>

export const StrategyList: Story = {
  render: () => <RouterProvider router={createHeaderRouter('/strategies')} />,
}

export const StrategyHome: Story = {
  render: () => (
    <RouterProvider router={createHeaderRouter('/strategies/semi-swing')} />
  ),
}

export const Portfolio: Story = {
  render: () => <RouterProvider router={createHeaderRouter('/portfolio')} />,
}
