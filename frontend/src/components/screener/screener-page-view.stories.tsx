import type { Meta, StoryObj } from '@storybook/react-vite'
import { RouterProvider } from '@tanstack/react-router'
import type { ComponentProps } from 'react'

import { ScreenerPageView } from '#components/screener/screener-page-view'
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
  {
    id: 'demo-code-2',
    name: '見本工業',
    market: 'スタンダード',
    sector: '架空機械',
    close: 1540,
    per: 12.4,
    pbr: 1.1,
    roe: 7.2,
    dividend_yield: 2.6,
    market_cap: 820,
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
  sectorOptions: [
    { key: 'sector-sample', name: 'サンプル業' },
    { key: 'sector-example', name: '架空機械' },
  ],
  sectorOptionsPending: false,
  sectorOptionsError: false,
  status: 'results',
  onApplyFilters: () => {},
  onSort: () => {},
  onSortOrder: () => {},
  onLoadMore: () => {},
} satisfies ComponentProps<typeof ScreenerPageView>

const meta = {
  title: 'Screener/ScreenerPageView',
  component: ScreenerPageView,
  parameters: { layout: 'fullscreen' },
  render: (viewArgs) => (
    <RouterProvider
      router={createStoryRouter(
        () => (
          <div className="p-4 md:p-6">
            <ScreenerPageView {...viewArgs} />
          </div>
        ),
        { paths: ['/stocks/$stockId'] },
      )}
    />
  ),
} satisfies Meta<typeof ScreenerPageView>

export default meta
type Story = StoryObj<typeof meta>

export const Results: Story = {
  name: '条件に一致する銘柄を表とカードで表示します。',
  args,
}

export const MobileResults: Story = {
  name: 'スマートフォンでは銘柄をカードで表示します。',
  args,
  globals: { viewport: { value: 'mobileScreener' } },
  parameters: {
    viewport: {
      options: {
        mobileScreener: {
          name: 'Mobile screener',
          styles: { width: '390px', height: '700px' },
          type: 'mobile',
        },
      },
    },
    screenshot: { viewport: { width: 390, height: 700 } },
  },
}

export const Empty: Story = {
  name: '該当する銘柄がないときに条件の見直しを案内します。',
  args: {
    ...args,
    data: { items: [], total_count: 0, as_of: '2099-04-15', page: 1 },
    stocks: [],
  },
}

export const Loading: Story = {
  name: '銘柄一覧を読み込んでいます。',
  args: { ...args, data: undefined, stocks: [], status: 'loading' },
}

export const Error: Story = {
  name: '銘柄の取得に失敗した状態を表示します。',
  args: {
    ...args,
    data: undefined,
    stocks: [],
    status: 'error',
    errorMessage: '銘柄の取得に失敗しました。もう一度お試しください。',
  },
}

export const MobileFilters: Story = {
  name: 'スマートフォン向けの全画面条件パネルを表示します。',
  args: { ...args, mobileFiltersOpen: true, search: { markets: ['プライム'] } },
  globals: { viewport: { value: 'mobileScreener' } },
  parameters: {
    viewport: {
      options: {
        mobileScreener: {
          name: 'Mobile screener',
          styles: { width: '390px', height: '700px' },
          type: 'mobile',
        },
      },
    },
    screenshot: { viewport: { width: 390, height: 700 } },
  },
}
