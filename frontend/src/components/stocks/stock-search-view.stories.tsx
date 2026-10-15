import type { Meta, StoryObj } from '@storybook/react-vite'
import { RouterProvider } from '@tanstack/react-router'
import type { ComponentProps } from 'react'

import { StockSearchView } from '#components/stocks/stock-search-view'
import type { components } from '#lib/api/schema.gen'
import { createStoryRouter } from '#storybook/story-router'

const sampleStock: components['schemas']['Stock'] = {
  id: 'demo-code',
  name: 'デモ商事',
  created_at: '2026-01-01T00:00:00Z',
  updated_at: '2026-01-01T00:00:00Z',
}

const args = {
  open: true,
  onOpenChange: () => {},
  query: '',
  onQueryChange: () => {},
  stocks: [],
  isPending: false,
  isError: false,
} satisfies ComponentProps<typeof StockSearchView>

const meta = {
  title: 'Stocks/StockSearchView',
  component: StockSearchView,
  parameters: { layout: 'padded' },
  render: (viewArgs) => (
    <RouterProvider
      router={createStoryRouter(
        () => (
          <StockSearchView {...viewArgs} />
        ),
        {
          paths: ['/stocks/$stockId', '/screener'],
        },
      )}
    />
  ),
} satisfies Meta<typeof StockSearchView>

export default meta
type Story = StoryObj<typeof meta>

export const Closed: Story = {
  name: 'ヘッダーに検索ボタンを表示します。',
  args: { ...args, open: false },
}

export const Open: Story = {
  name: '検索を開き、入力を促します。',
  args,
}

export const Pending: Story = {
  name: '銘柄を検索しています。',
  args: { ...args, query: 'デモ', isPending: true },
}

export const Error: Story = {
  name: '検索に失敗した状態を表示します。',
  args: { ...args, query: 'デモ', isError: true },
}

export const NoResults: Story = {
  name: '一致する銘柄がない状態を表示します。',
  args: { ...args, query: '見つからない銘柄' },
}

export const Results: Story = {
  name: '検索結果に銘柄名とコードを表示します。',
  args: { ...args, query: 'デモ', stocks: [sampleStock] },
}
