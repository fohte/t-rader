import type { Meta, StoryObj } from '@storybook/react-vite'
import { RouterProvider } from '@tanstack/react-router'

import { Header } from '#components/strategy-shell/header'
import { createStoryRouter } from '#storybook/story-router'
import { StrategySwitcherQueryDecorator } from '#storybook/strategy-switcher-mock'

function createHeaderRouter(initialPath: string, menuOpen = false) {
  return createStoryRouter(() => <Header menuOpen={menuOpen} />, {
    paths: [
      '/strategies',
      '/strategies/$id/performance',
      '/strategies/$id/runs',
      '/strategies/$id/indicators',
      '/strategies/$id/settings',
      '/',
      '/portfolio',
      '/trades',
      '/paper-trading',
      '/paper-trading/$accountId',
      '/notes',
      '/annotations',
      '/annotations/$annoId',
      '/runs',
      '/indicators',
      '/note-versions/pending',
      '/calendar',
      '/settings',
      '/settings/rss-feeds',
      '/charts/$instrumentId',
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
  name: 'shows the menu navigation active on the strategy list page.',
  render: () => <RouterProvider router={createHeaderRouter('/strategies')} />,
}

export const StrategyHome: Story = {
  name: 'shows the header on a strategy performance page.',
  render: () => (
    <RouterProvider
      router={createHeaderRouter('/strategies/demo-strategy/performance')}
    />
  ),
}

export const StrategyRuns: Story = {
  name: 'shows the strategy run history tab as active.',
  render: () => (
    <RouterProvider
      router={createHeaderRouter('/strategies/demo-strategy/runs')}
    />
  ),
}

export const StrategyIndicators: Story = {
  name: 'shows the strategy indicators tab as active.',
  render: () => (
    <RouterProvider
      router={createHeaderRouter('/strategies/demo-strategy/indicators')}
    />
  ),
}

export const Portfolio: Story = {
  name: 'shows the header with portfolio navigation active.',
  render: () => <RouterProvider router={createHeaderRouter('/portfolio')} />,
}

export const Trades: Story = {
  name: 'shows assets navigation active on the trade history page.',
  render: () => <RouterProvider router={createHeaderRouter('/trades')} />,
}

export const MenuOpen: Story = {
  name: 'shows the menu open on a global settings page.',
  render: () => (
    <RouterProvider router={createHeaderRouter('/settings/rss-feeds', true)} />
  ),
}
