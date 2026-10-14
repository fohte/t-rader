import { RouterProvider } from '@tanstack/react-router'
import { cleanup, render, screen, within } from '@testing-library/react'
import { afterEach, describe, expect, it } from 'vitest'

import { DashboardRecentTrades } from '#components/dashboard/dashboard-recent-trades'
import type { components } from '#lib/api/schema.gen'
import { createStoryRouter } from '#storybook/story-router'

type Trade = components['schemas']['TradeListItem']

afterEach(cleanup)

const trade: Trade = {
  id: 'fake-trade-a',
  strategy_id: 'fake-strategy-a',
  symbol: '0000',
  side: 'buy',
  qty: 100,
  price: 3200,
  fee: 0,
  date: '2026-10-08',
  source: 'manual',
  note: null,
  note_count: 1,
  created_at: '2026-10-08T02:00:00Z',
  updated_at: '2026-10-08T02:00:00Z',
}

describe('DashboardRecentTrades', () => {
  it('links each reflection action to its trade detail', async () => {
    const router = createStoryRouter(
      () => (
        <DashboardRecentTrades trades={[trade]} stocks={[]} isError={false} />
      ),
      { paths: ['/trades', '/trades/$tradeId', '/stocks/$stockId'] },
    )
    render(<RouterProvider router={router} />)

    const links = await screen.findAllByRole('link', {
      name: '振り返る →',
      hidden: true,
    })

    expect(links.map((link) => link.getAttribute('href'))).toEqual([
      '/trades/fake-trade-a',
      '/trades/fake-trade-a',
    ])
  })

  it('shows the four most recent trades in date and creation order', async () => {
    const trades: Trade[] = [
      trade,
      {
        ...trade,
        id: 'fake-trade-b',
        date: '2026-10-11',
        created_at: '2026-10-11T01:00:00Z',
      },
      {
        ...trade,
        id: 'fake-trade-c',
        date: '2026-10-10',
        created_at: '2026-10-10T01:00:00Z',
      },
      {
        ...trade,
        id: 'fake-trade-d',
        date: '2026-10-11',
        created_at: '2026-10-11T02:00:00Z',
      },
      {
        ...trade,
        id: 'fake-trade-e',
        date: '2026-10-09',
        created_at: '2026-10-09T01:00:00Z',
      },
    ]
    const router = createStoryRouter(
      () => (
        <DashboardRecentTrades trades={trades} stocks={[]} isError={false} />
      ),
      { paths: ['/trades', '/trades/$tradeId', '/stocks/$stockId'] },
    )
    render(<RouterProvider router={router} />)
    const table = await screen.findByRole('table', { hidden: true })
    const links = within(table).getAllByRole('link', {
      name: '振り返る →',
      hidden: true,
    })

    expect(links.map((link) => link.getAttribute('href'))).toEqual([
      '/trades/fake-trade-d',
      '/trades/fake-trade-b',
      '/trades/fake-trade-c',
      '/trades/fake-trade-e',
    ])
  })
})
