import type { Meta, StoryObj } from '@storybook/react-vite'
import { RouterProvider } from '@tanstack/react-router'

import { lowerPaneIndicatorBars } from '#components/stock-page/stock-chart.fixtures'
import { StockChartPageView } from '#components/stock-page/stock-chart-page-view'
import type { components } from '#lib/api/schema.gen'
import type { ChartIndicatorConfig } from '#lib/chart-indicator-settings'
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

const lowerPaneIndicators: ChartIndicatorConfig[] = [
  { id: 'indicator-rsi', kind: 'rsi', period: 3, color: '#38bdf8' },
  {
    id: 'indicator-macd',
    kind: 'macd',
    fastPeriod: 2,
    slowPeriod: 5,
    signalPeriod: 3,
    color: '#34d399',
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
    horizontalLineDrawingEnabled: false,
    pendingHorizontalLine: null,
    isSavingHorizontalLine: false,
    horizontalLineSaveError: null,
    onToggleHorizontalLineDrawing: () => undefined,
    onPlaceHorizontalLine: () => undefined,
    onSaveHorizontalLine: () => undefined,
    onCancelHorizontalLine: () => undefined,
    trendLineDrawingEnabled: false,
    pendingTrendLineStart: null,
    pendingTrendLinePreviewPoint: null,
    pendingTrendLine: null,
    isSavingTrendLine: false,
    trendLineSaveError: null,
    onToggleTrendLineDrawing: () => undefined,
    onPlaceTrendLinePoint: () => undefined,
    onSaveTrendLine: () => undefined,
    onCancelTrendLine: () => undefined,
    isDeletingAnnotation: false,
    annotationDeleteError: null,
    onDeleteAnnotation: () => undefined,
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

export const WithLowerPaneIndicators: Story = {
  name: 'RSI と MACD の下段を追加した全画面チャートを表示する。',
  args: { bars: lowerPaneIndicatorBars, chartIndicators: lowerPaneIndicators },
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

export const DrawingHorizontalLine: Story = {
  name: '水平線ツールを選び、チャート上のクリックを待つ。',
  args: { horizontalLineDrawingEnabled: true },
}

export const DrawingTrendLine: Story = {
  name: 'トレンドラインツールを選び、2 点の配置を待つ。',
  args: { trendLineDrawingEnabled: true },
}

export const PreviewingTrendLine: Story = {
  name: '始点を置き、カーソル位置までの線を確認する。',
  args: {
    trendLineDrawingEnabled: true,
    pendingTrendLineStart: {
      timestamp: '2030-04-01T06:00:00Z',
      price: 3390,
    },
    pendingTrendLinePreviewPoint: {
      timestamp: '2030-04-02T06:00:00Z',
      price: 3120,
    },
  },
}

export const EditingHorizontalLine: Story = {
  name: '置いた水平線の種別キーとメモを入力する。',
  args: {
    annotations: [
      {
        id: 'annotation-sample',
        created_by_kind: 'human',
        price: 3110,
        price_start: null,
        status: 'approved',
        target_kind: 'sample-kind',
        text: '過去の値動きで反応した水準を確認する。',
        timestamp: '2030-04-02T06:00:00Z',
        timestamp_start: null,
      },
    ],
    pendingHorizontalLine: {
      timestamp: '2030-04-02T06:00:00Z',
      price: 3140,
    },
  },
}

export const EditingTrendLine: Story = {
  name: '置いたトレンドラインの種別キーとメモを入力する。',
  args: {
    pendingTrendLine: {
      start: { timestamp: '2030-04-01T06:00:00Z', price: 3390 },
      end: { timestamp: '2030-04-02T06:00:00Z', price: 3120 },
    },
  },
}
