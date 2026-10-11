import { describe, expect, it } from 'vitest'

import type { components } from '#lib/api/schema.gen'
import { resolvePaperTradingAccountState } from '#lib/paper-trading-account-state'

type PaperAccountStats = components['schemas']['PaperAccountStats']

const accountStats: PaperAccountStats = {
  account_id: '00000000-0000-4000-8000-000000000101',
  account_name: '検証口座A',
  strategy_id: '00000000-0000-4000-8000-000000000201',
  purpose: '設定A',
  started_on: '2026-01-15',
  initial_cash_jpy: 2_000_000,
  benchmark_stock_id: null,
  as_of: '2026-06-30',
  total_assets_jpy: 2_000_000,
  return_since_start: 0,
  benchmark_return: null,
  closed_trade_count: 0,
  win_rate: null,
  average_win_excess_return: null,
  average_loss_excess_return: null,
  unrealized_pnl_jpy: 0,
}

describe('resolvePaperTradingAccountState', () => {
  it.each([
    {
      name: 'missing account takes precedence over portfolio failure',
      stats: [],
      portfolioQuery: { isPending: false, isError: true },
      expected: { state: 'not-found' },
    },
    {
      name: 'existing account waits for its portfolio',
      stats: [accountStats],
      portfolioQuery: { isPending: true, isError: false },
      expected: { state: 'loading' },
    },
    {
      name: 'existing account reports portfolio failure',
      stats: [accountStats],
      portfolioQuery: { isPending: false, isError: true },
      expected: { state: 'error' },
    },
    {
      name: 'existing account returns its stats when portfolio is ready',
      stats: [accountStats],
      portfolioQuery: { isPending: false, isError: false },
      expected: { state: 'ready', stats: accountStats },
    },
  ] as const)('$name', ({ stats, portfolioQuery, expected }) => {
    expect(
      resolvePaperTradingAccountState(
        stats,
        accountStats.account_id,
        portfolioQuery,
      ),
    ).toEqual(expected)
  })
})
