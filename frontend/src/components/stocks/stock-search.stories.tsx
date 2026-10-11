import type { Meta, StoryObj } from '@storybook/react-vite'
import { RouterProvider } from '@tanstack/react-router'

import { StockSearchView } from '#components/stocks/stock-search'
import type { components } from '#lib/api/schema.gen'
import { createStoryRouter } from '#storybook/story-router'

const sampleStock: components['schemas']['Stock'] = {
  id: 'demo-code',
  name: 'デモ商事',
  created_at: '2026-01-01T00:00:00Z',
  updated_at: '2026-01-01T00:00:00Z',
}

const meta = {
  title: 'Stocks/StockSearch',
  component: StockSearchView,
  parameters: { layout: 'padded' },
  render: (args) => (
    <RouterProvider
      router={createStoryRouter(
        () => (
          <StockSearchView {...args} />
        ),
        {
          paths: ['/charts/$instrumentId'],
        },
      )}
    />
  ),
} satisfies Meta<typeof StockSearchView>

export default meta
type Story = StoryObj<typeof meta>

export const Closed: Story = {
  name: 'ヘッダーに検索ボタンを表示します。',
  args: {
    open: false,
    onOpenChange: () => {},
    query: '',
    onQueryChange: () => {},
    stocks: [],
    isPending: false,
    isError: false,
  },
}

export const Open: Story = {
  name: '検索を開き、入力を促します。',
  args: {
    open: true,
    onOpenChange: () => {},
    query: '',
    onQueryChange: () => {},
    stocks: [],
    isPending: false,
    isError: false,
  },
}

export const Results: Story = {
  name: '検索結果に銘柄名とコードを表示します。',
  args: {
    open: true,
    onOpenChange: () => {},
    query: 'デモ',
    onQueryChange: () => {},
    stocks: [sampleStock],
    isPending: false,
    isError: false,
  },
}
