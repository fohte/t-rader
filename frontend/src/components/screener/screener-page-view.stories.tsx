import type { Meta, StoryObj } from '@storybook/react-vite'
import { RouterProvider } from '@tanstack/react-router'
import type { ComponentProps } from 'react'

import { ScreenerPageView } from '#components/screener/screener-page-view'
import type { components } from '#lib/api/schema.gen'
import type { SavedScreen } from '#lib/screener-search'
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

const savedScreen: SavedScreen = {
  id: 'saved-screen-a',
  name: '試作条件',
  created_at: '2099-01-01T00:00:00Z',
  updated_at: '2099-01-02T00:00:00Z',
  filters: {
    markets: ['プライム'],
    sectors: [],
    per: { min: null, max: 12 },
    pbr: { min: null, max: 1 },
    roe: null,
    dividend_yield: { min: 3, max: null },
    market_cap: { min: 100, max: null },
  },
  sort: { field: 'market_cap', direction: 'desc' },
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
  loadMoreError: false,
  onApplyFilters: () => {},
  onSort: () => {},
  onSortOrder: () => {},
  onLoadMore: () => {},
  onRetryLoadMore: () => {},
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

export const AdditionalPageError: Story = {
  name: '追加の取得に失敗しても読み込み済みの銘柄を表示します。',
  args: { ...args, loadMoreError: true },
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

export const NoSavedScreens: Story = {
  name: '保存した条件がない状態で条件の保存を案内します。',
  args: { ...args, savedScreens: [] },
}

export const SelectedSavedScreen: Story = {
  name: '保存した条件を読み込んだ状態を表示します。',
  args: {
    ...args,
    savedScreens: [savedScreen],
    search: {
      screen: savedScreen.id,
      markets: ['プライム'],
      max_per: 12,
      max_pbr: 1,
      min_dividend_yield: 3,
      min_market_cap: 100,
      sort_by: 'market_cap',
      sort_order: 'desc',
    },
  },
}

export const SavedScreensMenuOpen: Story = {
  name: '保存した条件の選択メニューを開いています。',
  args: {
    ...SelectedSavedScreen.args,
    savedScreensMenuOpen: true,
  },
}

export const EditedSavedScreen: Story = {
  name: '保存した条件の値を変えて上書き保存できる状態です。',
  args: {
    ...SelectedSavedScreen.args,
    search: {
      ...SelectedSavedScreen.args.search,
      max_per: 10,
      sort_by: 'per',
      sort_order: 'asc',
    },
  },
}

export const SaveDialog: Story = {
  name: '条件に名前を付けて保存する dialog を表示します。',
  args: {
    ...SelectedSavedScreen.args,
    savedScreenDialog: { type: 'save' },
  },
}

export const SaveError: Story = {
  name: '名前が重複したときに保存エラーを表示します。',
  args: {
    ...SelectedSavedScreen.args,
    savedScreenDialog: { type: 'save' },
    savedScreenMutationError: '同じ名前の保存条件がすでにあります。',
  },
}

export const MobileSavedScreen: Story = {
  name: 'スマートフォンで保存した条件と結果を表示します。',
  args: SelectedSavedScreen.args,
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

export const MobileSavedScreensMenuOpen: Story = {
  name: 'スマートフォンで保存した条件のメニューを開いています。',
  args: { ...MobileSavedScreen.args, savedScreensMenuOpen: true },
  globals: { viewport: { value: 'mobileScreener' } },
  parameters: MobileSavedScreen.parameters,
}

export const MobileEditedSavedScreensMenuOpen: Story = {
  name: 'スマートフォンで変更した条件を上書き保存できます。',
  args: {
    ...MobileSavedScreen.args,
    savedScreensMenuOpen: true,
    search: {
      ...MobileSavedScreen.args.search,
      max_per: 10,
      sort_by: 'per',
      sort_order: 'asc',
    },
  },
  globals: { viewport: { value: 'mobileScreener' } },
  parameters: MobileSavedScreen.parameters,
}
