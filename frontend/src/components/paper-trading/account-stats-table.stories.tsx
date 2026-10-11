import type { Meta, StoryObj } from '@storybook/react-vite'
import { RouterProvider } from '@tanstack/react-router'

import { PaperAccountStatsTable } from '#components/paper-trading/account-stats-table'
import type { components } from '#lib/api/schema.gen'
import { createStoryRouter } from '#storybook/story-router'

type PaperAccountStats = components['schemas']['PaperAccountStats']

const stats: PaperAccountStats[] = [
  {
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
  },
  {
    account_id: '00000000-0000-4000-8000-000000000102',
    account_name: '検証口座B',
    strategy_id: '00000000-0000-4000-8000-000000000202',
    purpose: '設定B',
    started_on: '2026-02-20',
    initial_cash_jpy: 1_500_000,
    benchmark_stock_id: null,
    as_of: '2026-06-30',
    total_assets_jpy: 1_470_000,
    return_since_start: -0.02,
    benchmark_return: null,
    closed_trade_count: 0,
    win_rate: null,
    average_win_excess_return: null,
    average_loss_excess_return: null,
    unrealized_pnl_jpy: -30_000,
  },
]

const strategyNames: Record<string, string> = {
  '00000000-0000-4000-8000-000000000201': '架空戦略A',
  '00000000-0000-4000-8000-000000000202': '架空戦略B',
}

const noClosedTradeStats: PaperAccountStats = {
  account_id: '00000000-0000-4000-8000-000000000102',
  account_name: '検証口座B',
  strategy_id: '00000000-0000-4000-8000-000000000202',
  purpose: '設定B',
  started_on: '2026-02-20',
  initial_cash_jpy: 1_500_000,
  benchmark_stock_id: null,
  as_of: '2026-06-30',
  total_assets_jpy: 1_470_000,
  return_since_start: -0.02,
  benchmark_return: null,
  closed_trade_count: 0,
  win_rate: null,
  average_win_excess_return: null,
  average_loss_excess_return: null,
  unrealized_pnl_jpy: -30_000,
}

const meta = {
  title: 'PaperTrading/AccountStatsTable',
  component: PaperAccountStatsTable,
  parameters: { layout: 'padded' },
  args: { stats, strategyNames },
  decorators: [
    (Story) => (
      <RouterProvider
        router={createStoryRouter(
          () => (
            <Story />
          ),
          {
            paths: ['/paper-trading', '/paper-trading/$accountId'],
            initialPath: '/paper-trading',
          },
        )}
      />
    ),
  ],
} satisfies Meta<typeof PaperAccountStatsTable>

export default meta
type Story = StoryObj<typeof meta>

export const Comparison: Story = {
  name: '複数口座の成績を同じ表で比較する。',
}

export const NoAccounts: Story = {
  name: '口座がない場合に空状態を表示する。',
  args: { stats: [] },
}

export const NoClosedTrades: Story = {
  name: '決済がない口座の勝率をダッシュで表示する。',
  args: { stats: [noClosedTradeStats] },
}
