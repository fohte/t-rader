import type { Meta, StoryObj } from '@storybook/react-vite'
import { RouterProvider } from '@tanstack/react-router'

import { AppSidebar } from '#components/app-sidebar'
import { SidebarInset, SidebarProvider } from '#components/ui/sidebar'
import { createStoryRouter } from '#storybook/story-router'

function createSidebarStoryRouter(
  initialPath: string,
  content: React.ReactNode,
) {
  return createStoryRouter(
    () => (
      <SidebarProvider>
        <AppSidebar />
        <SidebarInset>
          <div className="p-4">{content}</div>
        </SidebarInset>
      </SidebarProvider>
    ),
    {
      paths: ['/', '/charts/$instrumentId', '/trades', '/notes'],
      initialPath,
    },
  )
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
    const router = createSidebarStoryRouter('/', 'ページコンテンツ')
    return <RouterProvider router={router} />
  },
}

export const OnWatchlistPage: Story = {
  render: () => {
    const router = createSidebarStoryRouter('/', 'ウォッチリストページ')
    return <RouterProvider router={router} />
  },
}

export const OnTradesPage: Story = {
  render: () => {
    const router = createSidebarStoryRouter('/trades', 'トレード履歴ページ')
    return <RouterProvider router={router} />
  },
}

export const OnNotesPage: Story = {
  render: () => {
    const router = createSidebarStoryRouter('/notes', 'ノートページ')
    return <RouterProvider router={router} />
  },
}
