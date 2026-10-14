import { RouterProvider } from '@tanstack/react-router'
import { cleanup, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it } from 'vitest'

import { PositionsTable } from '#components/portfolio/positions-table'
import type { components } from '#lib/api/schema.gen'
import { createStoryRouter } from '#storybook/story-router'

type Position = components['schemas']['PositionSummary']
type Stock = components['schemas']['Stock']

const positions: Position[] = [
  {
    symbol: 'demo-code',
    qty: 10,
    avg_cost: 100,
    cost_basis: 1000,
    realized_pnl: 0,
  },
  {
    symbol: 'demo-code-2',
    qty: 5,
    avg_cost: 200,
    cost_basis: 1000,
    realized_pnl: 0,
  },
]

const marketValuePositions: Position[] = [
  {
    symbol: 'demo-code',
    qty: 10,
    avg_cost: 100,
    cost_basis: 1000,
    current_price: 150,
    market_value: 1500,
    unrealized_pnl: 500,
    realized_pnl: 25,
  },
  {
    symbol: 'demo-code-2',
    qty: 5,
    avg_cost: 200,
    cost_basis: 1000,
    current_price: 180,
    market_value: 900,
    unrealized_pnl: -100,
    realized_pnl: -10,
  },
  {
    symbol: 'demo-code-3',
    qty: 2,
    avg_cost: 300,
    cost_basis: 600,
    current_price: null,
    market_value: null,
    unrealized_pnl: null,
    realized_pnl: 0,
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

afterEach(cleanup)

describe('PositionsTable', () => {
  it('銘柄名だけを日足チャートへのリンクとして表示する', async () => {
    const router = createStoryRouter(
      () => <PositionsTable positions={positions} stocks={stocks} />,
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

  it('時価順に並べ、時価がない保有を空欄として末尾に表示する', async () => {
    const router = createStoryRouter(
      () => (
        <PositionsTable
          positions={marketValuePositions}
          stocks={stocks}
          displayMarketValue
        />
      ),
      { paths: ['/charts/$instrumentId'] },
    )
    render(<RouterProvider router={router} />)
    await screen.findByRole('table')

    const rows = Array.from(
      document.querySelectorAll<HTMLTableRowElement>('tbody tr'),
    ).map((row) => Array.from(row.cells).map((cell) => cell.textContent.trim()))

    expect(rows).toEqual([
      [
        '架空商事demo-code',
        '¥1,500',
        '+¥500 +50.0%',
        '¥150',
        '10',
        '¥100',
        '¥1,000',
        '+¥25',
      ],
      [
        'demo-code-2',
        '¥900',
        '−¥100 −10.0%',
        '¥180',
        '5',
        '¥200',
        '¥1,000',
        '−¥10',
      ],
      ['demo-code-3', '—', '—', '—', '2', '¥300', '¥600', '¥0'],
    ])
  })
})
