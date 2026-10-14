import type { Meta, StoryObj } from '@storybook/react-vite'

import { StockPageChart } from '#components/stock-page/stock-page-chart'
import type { ChartAnnotation } from '#lib/annotation-chart-utils'
import type { components } from '#lib/api/schema.gen'

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

const meta = {
  title: 'StockPage/StockPageChart',
  component: StockPageChart,
  args: {
    bars,
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

export const Loading: Story = {
  name: 'チャートデータの読み込み中を表示する。',
  args: { isLoading: true },
}

export const Error: Story = {
  name: 'チャートデータの取得エラーを表示する。',
  args: { hasError: true },
}
