import type { Meta, StoryObj } from '@storybook/react-vite'
import { RouterProvider } from '@tanstack/react-router'

import { StockChartPageView } from '#components/stock-page/stock-chart-page-view'
import type { components } from '#lib/api/schema.gen'
import { createStoryRouter } from '#storybook/story-router'

type Bar = components['schemas']['Bar']

const bars: Bar[] = [
  {
    instrument_id: 'demo-code',
    timeframe: '1d',
    timestamp: '2030-04-01T06:00:00Z',
    open: 3000,
    high: 3100,
    low: 2980,
    close: 3060,
    volume: 10000,
  },
  {
    instrument_id: 'demo-code',
    timeframe: '1d',
    timestamp: '2030-04-02T06:00:00Z',
    open: 3060,
    high: 3140,
    low: 3030,
    close: 3110,
    volume: 12000,
  },
]

const meta = {
  title: 'StockPage/StockChartPageView',
  component: StockChartPageView,
  parameters: { layout: 'fullscreen' },
  decorators: [
    (Story) => (
      <RouterProvider
        router={createStoryRouter(
          () => (
            <Story />
          ),
          {
            paths: ['/stocks/$stockId'],
          },
        )}
      />
    ),
  ],
  args: {
    stockId: 'demo-code',
    stockName: 'デモ商事',
    bars,
    annotations: [],
    timeframe: '1d',
    onTimeframeChange: () => undefined,
    isMarketDepthOpen: false,
    onToggleMarketDepth: () => undefined,
    selectedAnnotationId: null,
    onSelectAnnotation: () => undefined,
    currency: 'JPY',
    isLoading: false,
    hasError: false,
    isAnnotationsLoading: false,
  },
} satisfies Meta<typeof StockChartPageView>

export default meta
type Story = StoryObj<typeof meta>

export const DailyChart: Story = {
  name: '銘柄名から銘柄ページへ戻れる全画面チャートを表示する。',
}

export const Loading: Story = {
  name: 'チャートデータの読み込み中を表示する。',
  args: { isLoading: true },
}

export const Error: Story = {
  name: 'チャートデータの取得に失敗した状態を表示する。',
  args: { hasError: true },
}

export const MarketDepthOpen: Story = {
  name: '板情報・歩み値パネルを開いた状態を表示する。',
  args: { isMarketDepthOpen: true },
}
