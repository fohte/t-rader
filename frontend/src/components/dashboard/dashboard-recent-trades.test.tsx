import { RouterProvider } from '@tanstack/react-router'
import { cleanup, render, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it, vi } from 'vitest'

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
  it('opens the existing trade note flow for the selected trade', async () => {
    const onReflect = vi.fn()
    const user = userEvent.setup()
    const router = createStoryRouter(
      () => (
        <DashboardRecentTrades
          trades={[trade]}
          stocks={[]}
          onReflect={onReflect}
          isError={false}
        />
      ),
      { paths: ['/trades', '/stocks/$stockId'] },
    )
    render(<RouterProvider router={router} />)

    const buttons = await screen.findAllByRole('button', {
      name: '振り返る →',
    })
    await user.click(
      buttons.at(0) ?? screen.getByRole('heading', { name: '直近の取引' }),
    )

    expect(onReflect.mock.calls).toEqual([[trade]])
  })

  it('shows the four most recent trades in date and creation order', async () => {
    const onReflect = vi.fn()
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
        <DashboardRecentTrades
          trades={trades}
          stocks={[]}
          onReflect={onReflect}
          isError={false}
        />
      ),
      { paths: ['/trades', '/stocks/$stockId'] },
    )
    render(<RouterProvider router={router} />)
    const table = await screen.findByRole('table', { hidden: true })
    const buttons = within(table).getAllByRole('button', {
      name: '振り返る →',
      hidden: true,
    })
    const user = userEvent.setup()
    for (const button of buttons) await user.click(button)

    expect(onReflect.mock.calls).toEqual([
      [trades[3]],
      [trades[1]],
      [trades[2]],
      [trades[4]],
    ])
  })
})
