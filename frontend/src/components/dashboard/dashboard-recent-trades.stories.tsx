import type { Meta, StoryObj } from '@storybook/react-vite'
import { RouterProvider } from '@tanstack/react-router'
import type { ComponentType } from 'react'

import { DashboardRecentTrades } from '#components/dashboard/dashboard-recent-trades'
import type { components } from '#lib/api/schema.gen'
import { createStoryRouter } from '#storybook/story-router'

type Trade = components['schemas']['TradeListItem']
type Stock = components['schemas']['Stock']

const stocks: Stock[] = [
  {
    id: '0000',
    name: 'デモ商事',
    market: null,
    sector_id: null,
    created_at: '2026-01-01T00:00:00Z',
    updated_at: '2026-01-01T00:00:00Z',
  },
]
const trades: Trade[] = [
  {
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
  },
]

function withRouter(Story: ComponentType) {
  return (
    <RouterProvider
      router={createStoryRouter(
        () => (
          <Story />
        ),
        {
          paths: ['/trades', '/trades/$tradeId', '/stocks/$stockId'],
        },
      )}
    />
  )
}

const meta = {
  title: 'Dashboard/DashboardRecentTrades',
  component: DashboardRecentTrades,
  decorators: [withRouter],
  args: { trades, stocks, isError: false },
} satisfies Meta<typeof DashboardRecentTrades>

export default meta
type Story = StoryObj<typeof meta>

export const Recent: Story = {
  name: 'shows recent trades with links to reflect on each one.',
}

export const Empty: Story = {
  name: 'shows an empty state when there are no trades.',
  args: { trades: [] },
}

export const Loading: Story = {
  name: 'shows placeholders while trades are loading.',
  args: { trades: null },
}

export const Error: Story = {
  name: 'shows an error message when trades cannot be loaded.',
  args: { trades: null, isError: true },
}
