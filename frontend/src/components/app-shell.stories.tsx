import type { Meta, StoryObj } from '@storybook/react-vite'
import { RouterProvider } from '@tanstack/react-router'

import { AppShell } from '#components/app-shell'
import { createStoryRouter } from '#storybook/story-router'

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
      () => (
        <AppShell>
          <div className="space-y-4">
            <h2 className="text-xl font-bold">ウォッチリスト</h2>
            <p className="text-muted-foreground">
              ここにウォッチリストの内容が表示されます。
            </p>
          </div>
        </AppShell>
      ),
      { paths: ['/', '/charts/$instrumentId'] },
    )
    return <RouterProvider router={router} />
  },
}

export const WithLongContent: Story = {
  render: () => {
    const router = createStoryRouter(
      () => (
        <AppShell>
          <div className="space-y-4">
            <h2 className="text-xl font-bold">ウォッチリスト</h2>
            {Array.from({ length: 50 }, (_, i) => (
              <p key={i} className="text-muted-foreground">
                アイテム {i + 1}: サンプルコンテンツ
              </p>
            ))}
          </div>
        </AppShell>
      ),
      { paths: ['/', '/charts/$instrumentId'] },
    )
    return <RouterProvider router={router} />
  },
}

export const WithChatSidebar: Story = {
  render: () => {
    const router = createStoryRouter(
      () => (
        <AppShell>
          <div className="space-y-4">
            <h2 className="text-xl font-bold">ウォッチリスト</h2>
            <p className="text-muted-foreground">
              右上の AI
              チャットボタンをクリックして、サイドバーの開閉を確認できます。
            </p>
          </div>
        </AppShell>
      ),
      { paths: ['/', '/charts/$instrumentId'] },
    )
    return <RouterProvider router={router} />
  },
}
