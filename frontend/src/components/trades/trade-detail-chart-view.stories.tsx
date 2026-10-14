import type { Meta, StoryObj } from '@storybook/react-vite'

import { TradeDetailChartView } from '#components/trades/trade-detail-chart-view'
import type { components } from '#lib/api/schema.gen'

type TradeDetail = components['schemas']['TradeDetailResponse']
type Trade = components['schemas']['TradeListItem']
type Bar = components['schemas']['Bar']

const trade: TradeDetail = {
  id: 'fake-trade-buy',
  strategy_id: 'fake-strategy',
  symbol: 'FIC-01',
  side: 'buy',
  qty: 10,
  price: 1000,
  fee: 10,
  date: '2030-01-02',
  source: 'manual',
  note: null,
  created_at: '2030-01-02T00:00:00Z',
  updated_at: '2030-01-02T00:00:00Z',
  remaining_qty: 10,
}

const tradeMarker: Trade = {
  id: trade.id,
  strategy_id: trade.strategy_id,
  symbol: trade.symbol,
  side: trade.side,
  qty: trade.qty,
  price: trade.price,
  fee: trade.fee,
  date: trade.date,
  source: trade.source,
  note: trade.note,
  note_count: 0,
  created_at: trade.created_at,
  updated_at: trade.updated_at,
}

const bars: Bar[] = [
  {
    instrument_id: trade.symbol,
    timeframe: '1d',
    timestamp: '2030-01-01T00:00:00Z',
    open: 980,
    high: 1020,
    low: 970,
    close: 1000,
    volume: 1200,
  },
  {
    instrument_id: trade.symbol,
    timeframe: '1d',
    timestamp: '2030-01-02T00:00:00Z',
    open: 1000,
    high: 1030,
    low: 990,
    close: 1010,
    volume: 1400,
  },
  {
    instrument_id: trade.symbol,
    timeframe: '1d',
    timestamp: '2030-01-03T00:00:00Z',
    open: 1010,
    high: 1020,
    low: 960,
    close: 970,
    volume: 1600,
  },
]

const meta = {
  title: 'Trades/TradeDetailChartView',
  component: TradeDetailChartView,
  args: {
    trade,
    currency: 'JPY',
    state: { status: 'ready', bars, trades: [tradeMarker] },
  },
} satisfies Meta<typeof TradeDetailChartView>

export default meta
type Story = StoryObj<typeof meta>

export const Loaded: Story = {
  name: 'shows daily bars and marks the selected purchase.',
}

export const Loading: Story = {
  name: 'shows a placeholder while daily bars are loading.',
  args: { state: { status: 'loading' } },
}

export const Empty: Story = {
  name: 'shows a message when the selected date range has no bars.',
  args: { state: { status: 'empty' } },
}

export const Error: Story = {
  name: 'shows an error when daily bars cannot be loaded.',
  args: { state: { status: 'error' } },
}
