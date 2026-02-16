import type { Meta, StoryObj } from '@storybook/react-vite'
import {
  createMemoryHistory,
  createRootRoute,
  createRoute,
  createRouter,
  RouterProvider,
} from '@tanstack/react-router'
import type { ReactNode } from 'react'

import { AppShell } from '@/components/app-shell'

function createStoryRouter(children: ReactNode) {
  const rootRoute = createRootRoute({
    component: () => <AppShell>{children}</AppShell>,
  })
  const indexRoute = createRoute({
    getParentRoute: () => rootRoute,
    path: '/',
    component: () => null,
  })
  const chartsRoute = createRoute({
    getParentRoute: () => rootRoute,
    path: '/charts/$instrumentId',
    component: () => null,
  })

  return createRouter({
    routeTree: rootRoute.addChildren([indexRoute, chartsRoute]),
    history: createMemoryHistory({ initialEntries: ['/'] }),
  })
}

const meta = {
  title: 'Components/AppShell',
  parameters: {
    layout: 'fullscreen',
  },
} satisfies Meta

export default meta
type Story = StoryObj<typeof meta>

export const Default: Story = {
  render: () => {
    const router = createStoryRouter(
      <div className="space-y-4">
        <h2 className="text-xl font-bold">ウォッチリスト</h2>
        <p className="text-muted-foreground">
          ここにウォッチリストの内容が表示されます。
        </p>
      </div>,
    )
    return <RouterProvider router={router} />
  },
}

export const WithLongContent: Story = {
  render: () => {
    const router = createStoryRouter(
      <div className="space-y-4">
        <h2 className="text-xl font-bold">ウォッチリスト</h2>
        {Array.from({ length: 50 }, (_, i) => (
          <p key={i} className="text-muted-foreground">
            アイテム {i + 1}: サンプルコンテンツ
          </p>
        ))}
      </div>,
    )
    return <RouterProvider router={router} />
  },
}
