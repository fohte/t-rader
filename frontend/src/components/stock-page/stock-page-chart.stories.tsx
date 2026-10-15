import type { Meta, StoryObj } from '@storybook/react-vite'
import { RouterProvider } from '@tanstack/react-router'

import { StockPageChart } from '#components/stock-page/stock-page-chart'
import type { ChartAnnotation } from '#lib/annotation-chart-utils'
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
  {
    instrument_id: 'demo-code',
    timeframe: '1d',
    timestamp: '2030-04-03T06:00:00Z',
    open: 3110,
    high: 3260,
    low: 3090,
    close: 3250,
    volume: 15000,
  },
]

const annotations: ChartAnnotation[] = [
  {
    id: 'demo-annotation',
    price: 3110,
    status: 'unread',
    target_kind: 'stock',
    text: '価格帯の上抜けを確認するデモ用アノテーションです。',
    timestamp: '2030-04-02T06:00:00Z',
    timestamp_start: null,
  },
]

const indicatorCloses = [
  3000, 3020, 3040, 3015, 3055, 3025, 3065, 3030, 3070, 3040, 3080, 3045,
]

const indicatorBars: Bar[] = indicatorCloses.map((close, index) => {
  const open = indicatorCloses[index - 1] ?? close - 8
  return {
    instrument_id: 'demo-code',
    timeframe: '1d',
    timestamp: new Date(Date.UTC(2030, 3, index + 1, 6)).toISOString(),
    open,
    high: Math.max(open, close) + 16,
    low: Math.min(open, close) - 18,
    close,
    volume: 10000 + index * 100,
  }
})

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
  title: 'StockPage/StockPageChart',
  component: StockPageChart,
  decorators: [
    (Story) => (
      <RouterProvider
        router={createStoryRouter(
          () => (
            <Story />
          ),
          {
            paths: ['/stocks/$stockId/chart'],
          },
        )}
      />
    ),
  ],
  args: {
    stockId: 'demo-code',
    bars,
    trades: [],
    annotations,
    selectedAnnotationId: 'demo-annotation',
    onSelectAnnotation: () => undefined,
    timeframe: '1d',
    onTimeframeChange: () => undefined,
    currency: 'JPY',
  },
} satisfies Meta<typeof StockPageChart>

export default meta
type Story = StoryObj<typeof meta>

export const DailyWithSelectedAnnotation: Story = {
  name: '選択中のアノテーション帯を日足チャートに表示する。',
}

export const WithLowerPaneIndicators: Story = {
  name: 'RSI と MACD の下段を追加した銘柄チャートを表示する。',
  args: { bars: indicatorBars, chartIndicators: lowerPaneIndicators },
}

export const DateSelected: Story = {
  name: '指定日の位置と四本値を表示する。',
  args: { selectedDate: '2030-04-02', selectedDateBar: bars[1] },
}

export const DateWithoutBar: Story = {
  name: '日足のない指定日には四本値がないことを表示する。',
  args: { selectedDate: '2030-04-04', selectedDateBar: null },
}

export const DateLoading: Story = {
  name: '指定した日の日足を読み込み中と表示する。',
  args: { selectedDate: '2030-04-02', selectedDateLoading: true },
}

export const DateError: Story = {
  name: '指定した日の日足の取得エラーを表示する。',
  args: { selectedDate: '2030-04-02', selectedDateError: true },
}

export const Loading: Story = {
  name: 'チャートデータの読み込み中を表示する。',
  args: { isLoading: true },
}

export const Error: Story = {
  name: 'チャートデータの取得エラーを表示する。',
  args: { hasError: true },
}
