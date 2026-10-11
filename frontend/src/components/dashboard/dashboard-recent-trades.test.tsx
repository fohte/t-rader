import { RouterProvider } from '@tanstack/react-router'
import { cleanup, render, screen } from '@testing-library/react'
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
        />
      ),
      { paths: ['/trades', '/charts/$instrumentId'] },
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
})
