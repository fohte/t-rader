import type { Meta, StoryObj } from '@storybook/react-vite'
import { RouterProvider } from '@tanstack/react-router'
import type { ComponentType } from 'react'
import { fn } from 'storybook/test'

import { PortfolioPageView } from '#components/portfolio/portfolio-page-view'
import type { PortfolioValuation } from '#components/portfolio/portfolio-valuation'
import type { components } from '#lib/api/schema.gen'
import type { PortfolioHistoryPoint } from '#lib/portfolio-history'
import { createStoryRouter } from '#storybook/story-router'

type PositionSummary = components['schemas']['PositionSummary']
type Stock = components['schemas']['Stock']

function withPortfolioRouter(Story: ComponentType) {
  return (
    <RouterProvider
      router={createStoryRouter(
        () => (
          <Story />
        ),
        { paths: ['/portfolio', '/trades', '/stocks/$stockId'] },
      )}
    />
  )
}

const stocks: Stock[] = [
  {
    id: 'demo-code',
    name: '架空商事',
    market: null,
    sector_id: null,
    created_at: '2099-01-01T00:00:00Z',
    updated_at: '2099-01-01T00:00:00Z',
  },
]

const positions: PositionSummary[] = [
  {
    symbol: 'demo-code',
    qty: 4,
    avg_cost: 1200,
    cost_basis: 4800,
    current_price: 1250,
    market_value: 5000,
    unrealized_pnl: 200,
    realized_pnl: 0,
  },
]

const history: PortfolioHistoryPoint[] = [
  {
    date: '2099-01-02',
    total_assets: 14800,
    stock_market_value: 4800,
    cash: 10000,
    investable_amount: 14800,
  },
  {
    date: '2099-01-03',
    total_assets: 15000,
    stock_market_value: 5000,
    cash: 10000,
    investable_amount: 14800,
  },
]

const valuation: PortfolioValuation = {
  costBasis: 4800,
  marketValue: 5000,
  unrealizedPnl: 200,
  unrealizedPnlRate: 4.16,
  cash: 10000,
  totalAssets: 15000,
  cashRatio: 66.7,
  investedRatio: 33.3,
  stats: [
    { label: '総資産 (時価)', value: '¥15,000' },
    { label: '評価額 (株式・時価)', value: '¥5,000', sub: '簿価 ¥4,800' },
    {
      label: '含み損益',
      value: '+¥200',
      sub: '+4.2%',
      cls: 'text-up',
    },
    { label: '現金', value: '¥10,000', sub: '66.7% 比率' },
    {
      label: '実現損益 (累計)',
      value: '¥0',
      cls: 'text-muted-foreground-strong',
    },
  ],
}

const meta = {
  title: 'Portfolio/PortfolioPageView',
  component: PortfolioPageView,
  decorators: [withPortfolioRouter],
  parameters: { layout: 'padded' },
  args: {
    summaryPending: false,
    valuation,
    history: { data: history, period: 'month', status: 'ready' },
    onHistoryPeriodChange: fn(),
    positions,
    stocks,
    allocation: [
      { key: 'demo-code', label: '架空商事', value: 5000, kind: 'position' },
      { key: 'cash', label: '現金', value: 10000, kind: 'cash' },
    ],
    priceObservedAt: '2099-01-03',
  },
} satisfies Meta<typeof PortfolioPageView>

export default meta
type Story = StoryObj<typeof meta>

export const Overview: Story = {
  name: 'shows portfolio value, history, holdings, and allocation.',
}

export const SummaryLoading: Story = {
  name: 'keeps the portfolio layout while summary data is loading.',
  args: { summaryPending: true },
}
