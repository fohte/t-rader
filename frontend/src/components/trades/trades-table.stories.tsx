import type { Meta, StoryObj } from '@storybook/react-vite'

import { TradesTable } from '#components/trades/trades-table'
import type { components } from '#lib/api/schema.gen'

type Trade = components['schemas']['TradeListItem']
type Strategy = components['schemas']['Strategy']
type Stock = components['schemas']['Stock']

const SWING_ID = '00000000-0000-0000-0000-000000000001'
const VALUE_ID = '00000000-0000-0000-0000-000000000002'

function strategyStub(id: string, name: string, sortOrder: number): Strategy {
  return {
    id,
    name,
    description: null,
    sort_order: sortOrder,
    created_at: '2026-01-01T00:00:00Z',
    updated_at: '2026-01-01T00:00:00Z',
  }
}

const strategies: Strategy[] = [
  strategyStub(SWING_ID, '検証用戦略 A', 0),
  strategyStub(VALUE_ID, '検証用戦略 B', 1),
]

const stocks: Stock[] = [
  {
    id: 'FICT1',
    name: '架空銘柄 A',
    market: null,
    sector_id: null,
    created_at: '2026-01-01T00:00:00Z',
    updated_at: '2026-01-01T00:00:00Z',
  },
  {
    id: 'FICT2',
    name: '架空銘柄 B',
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
    symbol: 'FICT1',
    side: 'buy',
    qty: 200,
    price: 1480,
    fee: 220,
    date: '2026-05-12',
    source: 'manual',
    note: null,
    note_count: 0,
    created_at: '2026-05-12T03:00:00Z',
    updated_at: '2026-05-12T03:00:00Z',
  },
  {
    id: 't2',
    strategy_id: SWING_ID,
    symbol: 'FICT1',
    side: 'sell',
    qty: 100,
    price: 1610,
    fee: 220,
    date: '2026-05-28',
    source: 'manual',
    note: null,
    note_count: 1,
    created_at: '2026-05-28T03:00:00Z',
    updated_at: '2026-05-28T03:00:00Z',
  },
  {
    id: 't3',
    strategy_id: VALUE_ID,
    symbol: 'FICT2',
    side: 'buy',
    qty: 100,
    price: 2810,
    fee: 320,
    date: '2026-04-10',
    source: 'csv',
    note: null,
    note_count: 0,
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
  name: 'shows trades with strategy labels and note actions.',
  args: {
    trades,
    strategies,
    stocks,
    showStrategy: true,
    onManageNotes: () => undefined,
  },
}

export const SingleStrategy: Story = {
  name: 'shows one strategy’s trades without a strategy column.',
  args: {
    trades: trades.filter((t) => t.strategy_id === SWING_ID),
    strategies,
    stocks,
    showStrategy: false,
    onManageNotes: () => undefined,
  },
}

export const Empty: Story = {
  name: 'shows the default empty state for a trade table.',
  args: { trades: [], strategies, stocks, showStrategy: true },
}

export const StaticNoteCount: Story = {
  name: 'shows note counts without trade-note management actions.',
  args: { trades, strategies, stocks, showStrategy: true },
}

export const EmptyUnlinked: Story = {
  name: 'shows an empty state when no unlinked trades remain.',
  args: {
    trades: [],
    strategies,
    stocks,
    showStrategy: true,
    emptyMessage: '未紐付けの取引はありません。',
  },
}
