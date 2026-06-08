import type { Meta, StoryObj } from '@storybook/react-vite'

import { PositionsTable } from '@/components/portfolio/positions-table'
import type { components } from '@/lib/api/schema.gen'

type Stock = components['schemas']['Stock']

const stocks: Stock[] = [
  {
    id: '3436',
    name: 'SUMCO',
    market: null,
    sector_id: null,
    created_at: '2026-01-01T00:00:00Z',
    updated_at: '2026-01-01T00:00:00Z',
  },
  {
    id: '7203',
    name: 'トヨタ自動車',
    market: null,
    sector_id: null,
    created_at: '2026-01-01T00:00:00Z',
    updated_at: '2026-01-01T00:00:00Z',
  },
]

const meta = {
  title: 'Portfolio/PositionsTable',
  component: PositionsTable,
  parameters: { layout: 'padded' },
} satisfies Meta<typeof PositionsTable>

export default meta
type Story = StoryObj<typeof meta>

export const Default: Story = {
  args: {
    stocks,
    positions: [
      {
        symbol: '3436',
        qty: 200,
        avg_cost: 1480,
        cost_basis: 296000,
        realized_pnl: 13000,
      },
      {
        symbol: '7203',
        qty: 100,
        avg_cost: 2810,
        cost_basis: 281000,
        realized_pnl: -4200,
      },
    ],
  },
}

export const Empty: Story = {
  args: { stocks, positions: [] },
}
