import type { Meta, StoryObj } from '@storybook/react-vite'

import { TRADE_DETAIL_CHART_BARS } from '#components/trades/trade-detail-chart.fixtures'
import { TradeDetailChartView } from '#components/trades/trade-detail-chart-view'
import { toTradeMarker } from '#components/trades/trade-detail-model'
import type { components } from '#lib/api/schema.gen'

type TradeDetail = components['schemas']['TradeDetailResponse']

const trade: TradeDetail = {
  id: 'fake-trade-buy',
  strategy_id: 'fake-strategy',
  symbol: 'FIC-01',
  side: 'buy',
  qty: 10,
  price: 1000,
  fee: 10,
  date: '2099-02-10',
  source: 'manual',
  note: null,
  created_at: '2099-02-10T00:00:00Z',
  updated_at: '2099-02-10T00:00:00Z',
  remaining_qty: 10,
}

const tradeMarker = toTradeMarker(trade)

const meta = {
  title: 'Trades/TradeDetailChartView',
  component: TradeDetailChartView,
  args: {
    trade,
    currency: 'JPY',
    state: {
      status: 'ready',
      bars: TRADE_DETAIL_CHART_BARS,
      trades: [tradeMarker],
    },
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
