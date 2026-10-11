import type { Meta, StoryObj } from '@storybook/react-vite'
import { RouterProvider } from '@tanstack/react-router'

import { PaperTradingAccountDetailView } from '#components/paper-trading/account-detail-view'
import type { components } from '#lib/api/schema.gen'
import { createStoryRouter } from '#storybook/story-router'

type PaperAccountStats = components['schemas']['PaperAccountStats']
type PaperAccountPortfolio = components['schemas']['PaperAccountPortfolio']

const stats: PaperAccountStats = {
  account_id: '00000000-0000-4000-8000-000000000101',
  account_name: '検証口座A',
  strategy_id: '00000000-0000-4000-8000-000000000201',
  purpose: '設定A',
  started_on: '2026-01-15',
  initial_cash_jpy: 2_000_000,
  benchmark_stock_id: 'SIM-001',
  as_of: '2026-06-30',
  total_assets_jpy: 2_250_000,
  return_since_start: 0.125,
  benchmark_return: 0.08,
  closed_trade_count: 8,
  win_rate: 0.625,
  average_win_excess_return: 0.11,
  average_loss_excess_return: -0.035,
  unrealized_pnl_jpy: 25_000,
}

const portfolio: PaperAccountPortfolio = {
  account_id: stats.account_id,
  account_name: stats.account_name,
  strategy_id: stats.strategy_id,
  purpose: stats.purpose,
  started_on: stats.started_on,
  as_of: stats.as_of,
  initial_cash_jpy: stats.initial_cash_jpy,
  cash_jpy: 1_100_000,
  positions: [
    {
      stock_id: 'SIM-001',
      qty: 100,
      avg_cost_jpy: 8_500,
      current_price_jpy: 8_750,
      market_value_jpy: 875_000,
      unrealized_pnl_jpy: 25_000,
    },
  ],
  orders: [
    {
      order_id: '00000000-0000-4000-8000-000000000301',
      stock_id: 'SIM-001',
      side: 'buy',
      qty: 100,
      note_id: '00000000-0000-4000-8000-000000000401',
      note_version_id: '00000000-0000-4000-8000-000000000501',
      ordered_at: '2026-06-20T01:30:00Z',
      outcome: 'filled',
      fill_date: '2026-06-20',
      fill_price_jpy: 8_500,
      reject_reason: null,
    },
    {
      order_id: '00000000-0000-4000-8000-000000000302',
      stock_id: 'SIM-002',
      side: 'sell',
      qty: 50,
      note_id: '00000000-0000-4000-8000-000000000402',
      note_version_id: '00000000-0000-4000-8000-000000000502',
      ordered_at: '2026-06-18T02:15:00Z',
      outcome: 'rejected',
      fill_date: null,
      fill_price_jpy: null,
      reject_reason: '保有数量を超えています',
    },
    {
      order_id: '00000000-0000-4000-8000-000000000303',
      stock_id: 'SIM-002',
      side: 'buy',
      qty: 25,
      note_id: '00000000-0000-4000-8000-000000000403',
      note_version_id: '00000000-0000-4000-8000-000000000503',
      ordered_at: '2026-06-17T03:45:00Z',
      outcome: null,
      fill_date: null,
      fill_price_jpy: null,
      reject_reason: null,
    },
  ],
}

const stockNames: Record<string, string> = {
  'SIM-001': '架空銘柄A',
  'SIM-002': '架空銘柄B',
}

const meta = {
  title: 'PaperTrading/AccountDetailView',
  component: PaperTradingAccountDetailView,
  parameters: { layout: 'padded' },
  args: {
    stats,
    portfolio,
    strategyName: '架空戦略A',
    stockNames,
  },
  decorators: [
    (Story) => (
      <RouterProvider
        router={createStoryRouter(
          () => (
            <Story />
          ),
          {
            paths: [
              '/paper-trading',
              '/paper-trading/$accountId',
              '/notes/$noteId',
            ],
            initialPath: '/paper-trading/00000000-0000-4000-8000-000000000101',
          },
        )}
      />
    ),
  ],
} satisfies Meta<typeof PaperTradingAccountDetailView>

export default meta
type Story = StoryObj<typeof meta>

export const Overview: Story = {
  name: '口座の成績・保有・注文をまとめて表示する。',
}

export const NoClosedTrades: Story = {
  name: '決済がない口座の勝率をダッシュで表示する。',
  args: {
    stats: {
      ...stats,
      account_id: '00000000-0000-4000-8000-000000000102',
      account_name: '検証口座B',
      strategy_id: '00000000-0000-4000-8000-000000000202',
      purpose: '設定B',
      benchmark_stock_id: null,
      benchmark_return: null,
      closed_trade_count: 0,
      win_rate: null,
      average_win_excess_return: null,
      average_loss_excess_return: null,
    },
    portfolio: {
      ...portfolio,
      account_id: '00000000-0000-4000-8000-000000000102',
      account_name: '検証口座B',
      strategy_id: '00000000-0000-4000-8000-000000000202',
      purpose: '設定B',
      positions: [],
      orders: [],
    },
    strategyName: '架空戦略B',
  },
}
