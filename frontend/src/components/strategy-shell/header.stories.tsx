import type { Meta, StoryObj } from '@storybook/react-vite'
import {
  createMemoryHistory,
  createRootRoute,
  createRoute,
  createRouter,
  RouterProvider,
} from '@tanstack/react-router'

import { Header } from '@/components/strategy-shell/header'

function createStoryRouter(initialPath: string) {
  const rootRoute = createRootRoute({ component: () => <Header /> })
  const strategiesRoute = createRoute({
    getParentRoute: () => rootRoute,
    path: '/strategies',
    component: () => null,
  })
  const strategyHomeRoute = createRoute({
    getParentRoute: () => rootRoute,
    path: '/strategies/$id',
    component: () => null,
  })
  const settingsRoute = createRoute({
    getParentRoute: () => rootRoute,
    path: '/strategies/$id/settings',
    component: () => null,
  })
  const portfolioRoute = createRoute({
    getParentRoute: () => rootRoute,
    path: '/portfolio',
    component: () => null,
  })
  const tradesRoute = createRoute({
    getParentRoute: () => rootRoute,
    path: '/trades',
    component: () => null,
  })
  return createRouter({
    routeTree: rootRoute.addChildren([
      strategiesRoute,
      strategyHomeRoute,
      settingsRoute,
      portfolioRoute,
      tradesRoute,
    ]),
    history: createMemoryHistory({ initialEntries: [initialPath] }),
  })
}

const meta = {
  title: 'StrategyShell/Header',
  parameters: { layout: 'fullscreen' },
} satisfies Meta

export default meta
type Story = StoryObj<typeof meta>

export const StrategyList: Story = {
  render: () => <RouterProvider router={createStoryRouter('/strategies')} />,
}

export const StrategyHome: Story = {
  render: () => (
    <RouterProvider router={createStoryRouter('/strategies/semi-swing')} />
  ),
}

export const Portfolio: Story = {
  render: () => <RouterProvider router={createStoryRouter('/portfolio')} />,
}
