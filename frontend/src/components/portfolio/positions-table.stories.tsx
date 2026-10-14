import type { Meta, StoryObj } from '@storybook/react-vite'
import { RouterProvider } from '@tanstack/react-router'
import type { ComponentType } from 'react'

import { PositionsTable } from '#components/portfolio/positions-table'
import type { components } from '#lib/api/schema.gen'
import { createStoryRouter } from '#storybook/story-router'

type Stock = components['schemas']['Stock']

const stocks: Stock[] = [
  {
    id: 'demo-code',
    name: '架空商事',
    market: null,
    sector_id: null,
    created_at: '2099-01-01T00:00:00Z',
    updated_at: '2099-01-01T00:00:00Z',
  },
  {
    id: 'demo-code-2',
    name: 'サンプル工業',
    market: null,
    sector_id: null,
    created_at: '2099-01-01T00:00:00Z',
    updated_at: '2099-01-01T00:00:00Z',
  },
]

function withChartRouter(Story: ComponentType) {
  return (
    <RouterProvider
      router={createStoryRouter(
        () => (
          <Story />
        ),
        {
          paths: ['/charts/$instrumentId'],
        },
      )}
    />
  )
}

const meta = {
  title: 'Portfolio/PositionsTable',
  component: PositionsTable,
  parameters: { layout: 'padded' },
  decorators: [(Story) => withChartRouter(Story)],
} satisfies Meta<typeof PositionsTable>

export default meta
type Story = StoryObj<typeof meta>

export const Default: Story = {
  name: 'shows market values and unrealized returns in descending value order.',
  args: {
    stocks,
    displayMarketValue: true,
    positions: [
      {
        symbol: 'demo-code',
        qty: 100,
        avg_cost: 1200,
        cost_basis: 120000,
        current_price: 1350,
        market_value: 135000,
        unrealized_pnl: 15000,
        realized_pnl: 1300,
      },
      {
        symbol: 'demo-code-2',
        qty: 50,
        avg_cost: 2400,
        cost_basis: 120000,
        current_price: 2280,
        market_value: 114000,
        unrealized_pnl: -6000,
        realized_pnl: -420,
      },
    ],
  },
}

export const MissingMarketValue: Story = {
  name: 'shows a dash when a holding has no observed market price.',
  args: {
    stocks,
    displayMarketValue: true,
    positions: [
      {
        symbol: 'demo-code',
        qty: 100,
        avg_cost: 1200,
        cost_basis: 120000,
        current_price: 1350,
        market_value: 135000,
        unrealized_pnl: 15000,
        realized_pnl: 1300,
      },
      {
        symbol: 'demo-code-2',
        qty: 50,
        avg_cost: 2400,
        cost_basis: 120000,
        current_price: null,
        market_value: null,
        unrealized_pnl: null,
        realized_pnl: 0,
      },
    ],
  },
}

export const CostBasisOnly: Story = {
  name: 'shows the cost basis columns without market value data.',
  args: {
    stocks,
    positions: [
      {
        symbol: 'demo-code',
        qty: 100,
        avg_cost: 1200,
        cost_basis: 120000,
        realized_pnl: 1300,
      },
    ],
  },
}

export const Empty: Story = {
  name: 'shows the positions table when the portfolio has no holdings.',
  args: { stocks, positions: [], displayMarketValue: true },
}
