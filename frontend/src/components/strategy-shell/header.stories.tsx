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
      '/strategies/$id/settings',
      '/',
      '/portfolio',
      '/trades',
      '/paper-trading',
      '/paper-trading/$accountId',
      '/notes',
      '/dashboards',
      '/dashboards/$key',
      '/annotations',
      '/annotations/$annoId',
      '/runs',
      '/review',
      '/calendar',
      '/settings',
      '/settings/rss-feeds',
      '/stocks/$stockId',
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

export const StrategyHomeMobileViewport: Story = {
  name: 'shows the arrow-only strategy back link on a phone viewport.',
  globals: { viewport: { value: 'mobileStrategyHeader' } },
  parameters: {
    viewport: {
      options: {
        mobileStrategyHeader: {
          name: 'Mobile strategy header',
          styles: { width: '390px', height: '700px' },
          type: 'mobile',
        },
      },
    },
    screenshot: {
      viewport: { width: 390, height: 700 },
    },
  },
  render: () => (
    <RouterProvider
      router={createHeaderRouter('/strategies/demo-strategy/performance')}
    />
  ),
}

export const StrategyRuns: Story = {
  name: 'shows the strategy execution tab as active.',
  render: () => (
    <RouterProvider
      router={createHeaderRouter('/strategies/demo-strategy/runs')}
    />
  ),
}

export const Portfolio: Story = {
  name: 'shows the header with portfolio navigation active.',
  render: () => <RouterProvider router={createHeaderRouter('/portfolio')} />,
}

export const MenuOpen: Story = {
  name: 'shows the menu open on a global settings page.',
  render: () => (
    <RouterProvider router={createHeaderRouter('/settings/rss-feeds', true)} />
  ),
}

export const DashboardPage: Story = {
  name: 'shows the dashboard navigation active on a saved dashboard page.',
  render: () => (
    <RouterProvider router={createHeaderRouter('/dashboards/sample-market')} />
  ),
}
