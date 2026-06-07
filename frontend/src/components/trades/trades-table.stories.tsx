import type { Meta, StoryObj } from '@storybook/react-vite'

import { TradesTable } from '@/components/trades/trades-table'
import type { components } from '@/lib/api/schema.gen'

type Trade = components['schemas']['Trade']
type Strategy = components['schemas']['Strategy']
type Stock = components['schemas']['Stock']

const SWING_ID = '00000000-0000-0000-0000-000000000001'
const VALUE_ID = '00000000-0000-0000-0000-000000000002'

const strategies: Strategy[] = [
  {
    id: SWING_ID,
    name: '半導体短期スイング',
    description: null,
    sort_order: 0,
    created_at: '2026-01-01T00:00:00Z',
    updated_at: '2026-01-01T00:00:00Z',
  },
  {
    id: VALUE_ID,
    name: '高配当バリュー長期',
    description: null,
    sort_order: 1,
    created_at: '2026-01-01T00:00:00Z',
    updated_at: '2026-01-01T00:00:00Z',
  },
]

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

const trades: Trade[] = [
  {
    id: 't1',
    strategy_id: SWING_ID,
    symbol: '3436',
    side: 'buy',
    qty: 200,
    price: 1480,
    fee: 220,
    date: '2026-05-12',
    source: 'manual',
    note: null,
    created_at: '2026-05-12T03:00:00Z',
    updated_at: '2026-05-12T03:00:00Z',
  },
  {
    id: 't2',
    strategy_id: SWING_ID,
    symbol: '3436',
    side: 'sell',
    qty: 100,
    price: 1610,
    fee: 220,
    date: '2026-05-28',
    source: 'manual',
    note: null,
    created_at: '2026-05-28T03:00:00Z',
    updated_at: '2026-05-28T03:00:00Z',
  },
  {
    id: 't3',
    strategy_id: VALUE_ID,
    symbol: '7203',
    side: 'buy',
    qty: 100,
    price: 2810,
    fee: 320,
    date: '2026-04-10',
    source: 'csv',
    note: null,
    created_at: '2026-04-10T03:00:00Z',
    updated_at: '2026-04-10T03:00:00Z',
  },
]

const meta = {
  title: 'Trades/TradesTable',
  component: TradesTable,
  parameters: { layout: 'padded' },
  args: {
    onEdit: () => undefined,
    onDelete: () => undefined,
  },
} satisfies Meta<typeof TradesTable>

export default meta
type Story = StoryObj<typeof meta>

export const WithStrategyColumn: Story = {
  args: { trades, strategies, stocks, showStrategy: true },
}

export const SingleStrategy: Story = {
  args: {
    trades: trades.filter((t) => t.strategy_id === SWING_ID),
    strategies,
    stocks,
    showStrategy: false,
  },
}

export const Empty: Story = {
  args: { trades: [], strategies, stocks, showStrategy: true },
}
