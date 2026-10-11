import type { Meta, StoryObj } from '@storybook/react-vite'
import { RouterProvider } from '@tanstack/react-router'

import { PaperTradingAccountsView } from '#components/paper-trading/accounts-view'
import type { components } from '#lib/api/schema.gen'
import { createStoryRouter } from '#storybook/story-router'

type PaperAccountStats = components['schemas']['PaperAccountStats']

const stats: PaperAccountStats[] = [
  {
    account_id: '00000000-0000-4000-8000-000000000111',
    account_name: '比較口座A',
    strategy_id: '00000000-0000-4000-8000-000000000211',
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
]

const strategyNames: Record<string, string> = {
  '00000000-0000-4000-8000-000000000211': '架空戦略A',
}

const meta = {
  title: 'PaperTrading/AccountsView',
  component: PaperTradingAccountsView,
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
} satisfies Meta<typeof PaperTradingAccountsView>

export default meta
type Story = StoryObj<typeof meta>

export const Comparison: Story = {
  name: '口座ごとの成績を専用ページで比較する。',
}

export const NoAccounts: Story = {
  name: '口座がないときも一覧ページの見出しを表示する。',
  args: { stats: [] },
}
