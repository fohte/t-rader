import type { Meta, StoryObj } from '@storybook/react-vite'
import { RouterProvider } from '@tanstack/react-router'
import type { ComponentProps } from 'react'

import { ScreenerResultsView } from '#components/screener/screener-results-view'
import type { components } from '#lib/api/schema.gen'
import { createStoryRouter } from '#storybook/story-router'

type ScreenerStock = components['schemas']['ScreenerStockResponse']
type ScreenerResponse = components['schemas']['ScreenerResponse']

const stocks: ScreenerStock[] = [
  {
    id: 'demo-code',
    name: '架空商事',
    market: 'プライム',
    sector: 'サンプル業',
    close: 3250,
    per: 8.2,
    pbr: 0.7,
    roe: 9.1,
    dividend_yield: 3.8,
    market_cap: 5140,
  },
]

const data: ScreenerResponse = {
  items: stocks,
  total_count: 142,
  as_of: '2099-04-15',
  page: 1,
}

const args = {
  search: {},
  data,
  stocks,
  status: 'results',
  loadMoreError: false,
  onSort: () => {},
  onSortOrder: () => {},
  onLoadMore: () => {},
  onRetryLoadMore: () => {},
} satisfies ComponentProps<typeof ScreenerResultsView>

const meta = {
  title: 'Screener/ScreenerResultsView',
  component: ScreenerResultsView,
  parameters: { layout: 'padded' },
  render: (viewArgs) => (
    <RouterProvider
      router={createStoryRouter(
        () => (
          <ScreenerResultsView {...viewArgs} />
        ),
        { paths: ['/stocks/$stockId'] },
      )}
    />
  ),
} satisfies Meta<typeof ScreenerResultsView>

export default meta
type Story = StoryObj<typeof meta>

export const Results: Story = {
  name: '件数と更新日、並べ替え可能な銘柄一覧を表示します。',
  args,
}

export const AgentRequest: Story = {
  name: '結果見出しから現在の条件でエージェントに依頼できます。',
  args: { ...args, onAskAgent: () => {} },
}

export const Empty: Story = {
  name: '一致する銘柄がないときに条件の見直しを促します。',
  args: {
    ...args,
    data: { items: [], total_count: 0, as_of: '2099-04-15', page: 1 },
    stocks: [],
  },
}

export const Loading: Story = {
  name: '検索中の結果プレースホルダーを表示します。',
  args: { ...args, data: undefined, stocks: [], status: 'loading' },
}

export const Error: Story = {
  name: '検索結果を取得できない状態を表示します。',
  args: {
    ...args,
    data: undefined,
    stocks: [],
    status: 'error',
    errorMessage: '銘柄の取得に失敗しました。もう一度お試しください。',
  },
}

export const AdditionalPageError: Story = {
  name: '追加の取得に失敗しても読み込み済みの銘柄と再試行を表示します。',
  args: { ...args, loadMoreError: true },
}
