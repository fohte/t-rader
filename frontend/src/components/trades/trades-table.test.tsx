import { RouterProvider } from '@tanstack/react-router'
import { cleanup, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it } from 'vitest'

import { TradesTable } from '#components/trades/trades-table'
import type { components } from '#lib/api/schema.gen'
import { createStoryRouter } from '#storybook/story-router'

type Trade = components['schemas']['TradeListItem']
type Strategy = components['schemas']['Strategy']
type Stock = components['schemas']['Stock']

const strategyId = '00000000-0000-0000-0000-000000000001'
const strategies: Strategy[] = [
  {
    id: strategyId,
    name: '検証用戦略',
    description: null,
    sort_order: 0,
    created_at: '2099-01-01T00:00:00Z',
    updated_at: '2099-01-01T00:00:00Z',
  },
]
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
const trades: Trade[] = [
  {
    id: 'demo-trade-1',
    strategy_id: strategyId,
    symbol: 'demo-code',
    side: 'buy',
    qty: 10,
    price: 100,
    fee: 0,
    date: '2099-01-02',
    source: 'manual',
    note: null,
    note_count: 0,
    created_at: '2099-01-02T00:00:00Z',
    updated_at: '2099-01-02T00:00:00Z',
  },
  {
    id: 'demo-trade-2',
    strategy_id: strategyId,
    symbol: 'demo-code-2',
    side: 'buy',
    qty: 5,
    price: 200,
    fee: 0,
    date: '2099-01-01',
    source: 'manual',
    note: null,
    note_count: 0,
    created_at: '2099-01-01T00:00:00Z',
    updated_at: '2099-01-01T00:00:00Z',
  },
]

afterEach(cleanup)

describe('TradesTable', () => {
  it('銘柄名だけを日足チャートへのリンクとして表示する', async () => {
    const router = createStoryRouter(
      () => (
        <TradesTable
          trades={trades}
          strategies={strategies}
          stocks={stocks}
          showStrategy
          onEdit={() => undefined}
          onDelete={() => undefined}
        />
      ),
      { paths: ['/charts/$instrumentId'] },
    )
    render(<RouterProvider router={router} />)
    await screen.findByRole('table')

    const rowLinks = Array.from(document.querySelectorAll('tbody tr')).map(
      (row) =>
        Array.from(row.querySelectorAll('a')).map((link) => ({
          name: link.textContent,
          href: link.getAttribute('href'),
        })),
    )

    expect(rowLinks).toEqual([
      [{ name: '架空商事', href: '/charts/demo-code' }],
      [{ name: 'demo-code-2', href: '/charts/demo-code-2' }],
    ])
  })
})
