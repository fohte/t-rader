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
    name: '架空商事 A',
    market: null,
    sector_id: null,
    created_at: '2026-01-01T00:00:00Z',
    updated_at: '2026-01-01T00:00:00Z',
  },
  {
    id: 'demo-code-2',
    name: '架空商事 B',
    market: null,
    sector_id: null,
    created_at: '2026-01-01T00:00:00Z',
    updated_at: '2026-01-01T00:00:00Z',
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
  name: 'lists current positions with cost basis and realized gains or losses.',
  args: {
    stocks,
    positions: [
      {
        symbol: 'demo-code',
        qty: 200,
        avg_cost: 1480,
        cost_basis: 296000,
        realized_pnl: 13000,
      },
      {
        symbol: 'demo-code-2',
        qty: 100,
        avg_cost: 2810,
        cost_basis: 281000,
        realized_pnl: -4200,
      },
    ],
  },
}

export const Empty: Story = {
  name: 'shows the positions table when the portfolio has no holdings.',
  args: { stocks, positions: [] },
}
