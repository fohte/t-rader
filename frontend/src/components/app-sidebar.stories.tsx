import type { Meta, StoryObj } from '@storybook/react-vite'
import {
  createMemoryHistory,
  createRootRoute,
  createRoute,
  createRouter,
  RouterProvider,
} from '@tanstack/react-router'

import { AppSidebar } from '@/components/app-sidebar'
import { SidebarInset, SidebarProvider } from '@/components/ui/sidebar'

function createStoryRouter(initialPath: string, content: React.ReactNode) {
  const rootRoute = createRootRoute({
    component: () => (
      <SidebarProvider>
        <AppSidebar />
        <SidebarInset>
          <div className="p-4">{content}</div>
        </SidebarInset>
      </SidebarProvider>
    ),
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
    history: createMemoryHistory({ initialEntries: [initialPath] }),
  })
}

const meta = {
  title: 'Components/AppSidebar',
  parameters: {
    layout: 'fullscreen',
  },
} satisfies Meta

export default meta
type Story = StoryObj<typeof meta>

export const Default: Story = {
  render: () => {
    const router = createStoryRouter('/', 'ページコンテンツ')
    return <RouterProvider router={router} />
  },
}

export const OnWatchlistPage: Story = {
  render: () => {
    const router = createStoryRouter('/', 'ウォッチリストページ')
    return <RouterProvider router={router} />
  },
}
