import type { Meta, StoryObj } from '@storybook/react-vite'

import { StrategyFilterBar } from '#components/trades/strategy-filter-bar'
import type { components } from '#lib/api/schema.gen'

type Strategy = components['schemas']['Strategy']
type Trade = components['schemas']['TradeListItem']

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

function tradeStub(strategyId: string, id: string): Trade {
  return {
    id,
    strategy_id: strategyId,
    symbol: 'FICT1',
    side: 'buy',
    qty: 100,
    price: 1500,
    fee: 220,
    date: '2026-05-01',
    source: 'manual',
    note: null,
    note_count: 0,
    created_at: '2026-05-01T00:00:00Z',
    updated_at: '2026-05-01T00:00:00Z',
  }
}

const trades: Trade[] = [
  tradeStub(SWING_ID, 't1'),
  { ...tradeStub(SWING_ID, 't2'), note_count: 1 },
  tradeStub(VALUE_ID, 't3'),
]

const meta = {
  title: 'Trades/StrategyFilterBar',
  component: StrategyFilterBar,
  parameters: { layout: 'padded' },
  args: {
    trades,
    strategies,
    value: 'all',
    onChange: () => {},
    unlinkedCount: 2,
    onlyUnlinked: false,
    onOnlyUnlinkedChange: () => {},
  },
} satisfies Meta<typeof StrategyFilterBar>

export default meta
type Story = StoryObj<typeof meta>

export const All: Story = {
  args: { value: 'all' },
}

export const SwingActive: Story = {
  args: { value: SWING_ID, unlinkedCount: 1 },
}

export const UnlinkedOnly: Story = {
  args: { onlyUnlinked: true },
}
