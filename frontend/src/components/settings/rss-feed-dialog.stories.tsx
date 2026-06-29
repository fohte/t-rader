import type { Meta, StoryObj } from '@storybook/react-vite'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'

import { RssFeedDialog } from '@/components/settings/rss-feed-dialog'

const queryClient = new QueryClient()

const meta = {
  title: 'Settings/RssFeedDialog',
  component: RssFeedDialog,
  decorators: [
    (Story) => (
      <QueryClientProvider client={queryClient}>
        <Story />
      </QueryClientProvider>
    ),
  ],
} satisfies Meta<typeof RssFeedDialog>

export default meta
type Story = StoryObj<typeof meta>

export const Create: Story = {
  args: {
    open: true,
    onOpenChange: () => {},
    feed: null,
  },
}

export const Edit: Story = {
  args: {
    open: true,
    onOpenChange: () => {},
    feed: {
      id: '00000000-0000-0000-0000-000000000001',
      source: 'bloomberg-jp',
      display_name: 'Bloomberg JP',
      url: 'https://feeds.bloomberg.co.jp/markets.xml',
      enabled: true,
      created_at: '2026-06-28T00:00:00Z',
      updated_at: '2026-06-28T00:00:00Z',
    },
  },
}
