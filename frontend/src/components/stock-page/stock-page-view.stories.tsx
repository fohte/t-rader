import type { Meta, StoryObj } from '@storybook/react-vite'
import { RouterProvider } from '@tanstack/react-router'

import { StockPageView } from '#components/stock-page/stock-page-view'
import type { components } from '#lib/api/schema.gen'
import { createStoryRouter } from '#storybook/story-router'

type Stock = components['schemas']['Stock']
type Position = components['schemas']['PositionSummary']
type Bar = components['schemas']['Bar']
type Annotation = components['schemas']['Annotation']
type Trade = components['schemas']['TradeListItem']

const stock: Stock = {
  id: 'demo-code',
  name: 'デモ商事',
  market: 'TSE',
  sector_id: null,
  product_category: null,
  created_at: '2030-04-01T00:00:00Z',
  updated_at: '2030-04-01T00:00:00Z',
}

const position: Position = {
  symbol: 'demo-code',
  qty: 100,
  avg_cost: 3200,
  cost_basis: 320000,
  realized_pnl: 4800,
}

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

const annotations: Annotation[] = [
  {
    id: 'demo-annotation',
    target_kind: 'stock',
    target_symbol: 'demo-code',
    timestamp: '2030-04-02T06:00:00Z',
    timestamp_start: null,
    price: 3110,
    text: '出来高を伴って直近高値を上抜けたデモ用メモです。',
    linked_note_id: null,
    status: 'unread',
    created_by_kind: 'llm',
    execution_step_id: null,
    execution_task_id: null,
    created_at: '2030-04-02T06:00:00Z',
    updated_at: '2030-04-02T06:00:00Z',
  },
]

const trades: Trade[] = [
  {
    id: 'demo-trade',
    strategy_id: '00000000-0000-0000-0000-000000000001',
    symbol: 'demo-code',
    side: 'buy',
    qty: 100,
    price: 3200,
    fee: 0,
    date: '2030-04-01',
    source: 'manual',
    note: null,
    note_count: 0,
    created_at: '2030-04-01T06:00:00Z',
    updated_at: '2030-04-01T06:00:00Z',
  },
]

const meta = {
  title: 'StockPage/StockPageView',
  component: StockPageView,
  parameters: { layout: 'fullscreen' },
  decorators: [
    (Story) => (
      <RouterProvider
        router={createStoryRouter(
          () => (
            <Story />
          ),
          {
            paths: ['/annotations/$annoId', '/stocks/$stockId/chart'],
          },
        )}
      />
    ),
  ],
  args: {
    stock,
    quote: {
      close: 3250,
      change: 50,
      changePercent: 1.56,
      timestamp: '2030-04-03T06:00:00Z',
    },
    quoteLoading: false,
    quoteError: false,
    bars,
    annotations,
    notes: [],
    trades,
    position,
    holdingsLoading: false,
    holdingsError: false,
    timeframe: '1d',
    onTimeframeChange: () => undefined,
    selectedAnnotationId: 'demo-annotation',
    onSelectAnnotation: () => undefined,
    activeTab: 'annotations',
    onTabChange: () => undefined,
    currency: 'JPY',
    chartLoading: false,
    chartError: false,
    annotationState: 'ready',
    noteState: 'ready',
    tradeState: 'ready',
  },
} satisfies Meta<typeof StockPageView>

export default meta
type Story = StoryObj<typeof meta>

export const DesktopPage: Story = {
  name: 'チャートと保有情報を並べた銘柄ページを表示する。',
}

export const WithoutPosition: Story = {
  name: '保有していない銘柄のチャートを全幅で表示する。',
  args: { position: null },
}

export const DataErrors: Story = {
  name: '終値と保有情報を取得できなかった状態を表示する。',
  args: {
    quote: null,
    quoteError: true,
    position: null,
    holdingsError: true,
  },
}
