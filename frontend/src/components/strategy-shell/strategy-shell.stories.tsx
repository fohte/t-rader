import type { Meta, StoryObj } from '@storybook/react-vite'
import {
  createMemoryHistory,
  createRootRoute,
  createRoute,
  createRouter,
  Outlet,
  RouterProvider,
} from '@tanstack/react-router'

import { StrategyShell } from '#components/strategy-shell/strategy-shell'

function createStoryRouter(initialPath: string) {
  const rootRoute = createRootRoute({
    component: () => (
      <StrategyShell>
        <Outlet />
      </StrategyShell>
    ),
  })
  const placeholders = [
    { path: '/strategies', label: '戦略一覧 placeholder' },
    { path: '/strategies/$id', label: '戦略ホーム placeholder' },
    { path: '/portfolio', label: 'ポートフォリオ placeholder' },
    { path: '/trades', label: '取引履歴 placeholder' },
  ]
  const children = placeholders.map((p) =>
    createRoute({
      getParentRoute: () => rootRoute,
      path: p.path,
      component: () => (
        <div className="font-mono text-sm text-[color:var(--color-text-secondary)]">
          {p.label}
        </div>
      ),
    }),
  )
  return createRouter({
    routeTree: rootRoute.addChildren(children),
    history: createMemoryHistory({ initialEntries: [initialPath] }),
  })
}

const meta = {
  title: 'StrategyShell/StrategyShell',
  parameters: { layout: 'fullscreen' },
} satisfies Meta

export default meta
type Story = StoryObj<typeof meta>

export const Default: Story = {
  render: () => <RouterProvider router={createStoryRouter('/strategies')} />,
}

export const StrategyHome: Story = {
  render: () => (
    <RouterProvider router={createStoryRouter('/strategies/semi-swing')} />
  ),
}
