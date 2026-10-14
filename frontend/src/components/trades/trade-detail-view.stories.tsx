import type { Meta, StoryObj } from '@storybook/react-vite'
import { RouterProvider } from '@tanstack/react-router'
import type { ComponentType } from 'react'

import { TRADE_DETAIL_CHART_BARS } from '#components/trades/trade-detail-chart.fixtures'
import { toTradeMarker } from '#components/trades/trade-detail-model'
import { TradeDetailView } from '#components/trades/trade-detail-view'
import type { components } from '#lib/api/schema.gen'
import { createStoryRouter } from '#storybook/story-router'

type TradeDetail = components['schemas']['TradeDetailResponse']
type Note = components['schemas']['Note']

const sellingTrade: TradeDetail = {
  id: 'fake-trade-sell',
  strategy_id: 'fake-strategy',
  symbol: 'FIC-01',
  side: 'sell',
  qty: 10,
  price: 1000,
  fee: 10,
  date: '2099-02-10',
  source: 'manual',
  note: null,
  created_at: '2099-02-10T00:00:00Z',
  updated_at: '2099-02-10T00:00:00Z',
  realized_pnl: 2200,
  matched_buys: [{ date: '2099-02-01', qty: 10, price: 780 }],
}

const buyingTrade: TradeDetail = {
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
  remaining_qty: 7,
}

const sellingTradeMarker = toTradeMarker(sellingTrade)
const buyingTradeMarker = toTradeMarker(buyingTrade)

const linkedNote: Note = {
  id: 'fake-note-human',
  version_id: 'fake-note-version-human',
  version_no: 1,
  is_current: true,
  title: '架空銘柄を買った理由',
  body_md: '架空の内容です。',
  frontmatter_json: {},
  resolved_price_references_json: {},
  graphs_json: [],
  tags: [],
  kind: null,
  status: 'approved',
  trigger: null,
  trigger_label: null,
  created_by_kind: 'human',
  created_at: '2099-02-10T00:00:00Z',
  updated_at: '2099-02-10T00:00:00Z',
  execution_id: null,
}

function withRouter(Story: ComponentType) {
  return (
    <RouterProvider
      router={createStoryRouter(
        () => (
          <Story />
        ),
        {
          paths: [
            '/trades',
            '/trades/$tradeId',
            '/stocks/$stockId',
            '/notes/$noteId',
          ],
        },
      )}
    />
  )
}

const meta = {
  title: 'Trades/TradeDetailView',
  component: TradeDetailView,
  decorators: [withRouter],
  args: {
    state: 'ready',
    trade: sellingTrade,
    chartState: {
      status: 'ready',
      bars: TRADE_DETAIL_CHART_BARS,
      trades: [sellingTradeMarker],
    },
    stockName: '架空銘柄',
    strategyName: '検証用戦略',
    linkedNotes: [linkedNote],
    notesPending: false,
    notesError: false,
    noteLinkError: null,
    onManageNotes: () => undefined,
    onWriteReflection: () => undefined,
  },
} satisfies Meta<typeof TradeDetailView>

export default meta
type Story = StoryObj<typeof meta>

export const SellingWithNotes: Story = {
  name: 'shows a sale with its matched purchase and linked notes.',
}

export const BuyingWithoutNotes: Story = {
  name: 'shows a purchase and an empty linked notes state.',
  args: {
    trade: buyingTrade,
    chartState: {
      status: 'ready',
      bars: TRADE_DETAIL_CHART_BARS,
      trades: [buyingTradeMarker],
    },
    linkedNotes: [],
  },
}

export const NotesLoading: Story = {
  name: 'shows a loading message while linked notes are loading.',
  args: { notesPending: true, linkedNotes: [] },
}

export const NotesLoadError: Story = {
  name: 'shows an error when linked notes cannot be loaded.',
  args: { notesError: true, linkedNotes: [] },
}

export const NoteLinkError: Story = {
  name: 'shows an error when a created note could not be linked.',
  args: {
    noteLinkError:
      'ノートを作成しましたが、この取引への紐付けに失敗しました。既存のノートを紐付ける操作から追加してください。',
  },
}

export const Loading: Story = {
  name: 'shows placeholders while trade details are loading.',
  args: { state: 'loading' },
}

export const Error: Story = {
  name: 'shows an error when trade details cannot be loaded.',
  args: { state: 'error' },
}
