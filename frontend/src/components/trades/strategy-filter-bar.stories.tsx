import type { Meta, StoryObj } from '@storybook/react-vite'
import { useState } from 'react'

import {
  type StrategyFilter,
  StrategyFilterBar,
} from '#components/trades/strategy-filter-bar'
import type { components } from '#lib/api/schema.gen'

type Strategy = components['schemas']['Strategy']
type Trade = components['schemas']['Trade']

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

function tradeStub(strategyId: string, id: string): Trade {
  return {
    id,
    strategy_id: strategyId,
    symbol: '3436',
    side: 'buy',
    qty: 100,
    price: 1500,
    fee: 220,
    date: '2026-05-01',
    source: 'manual',
    note: null,
    created_at: '2026-05-01T00:00:00Z',
    updated_at: '2026-05-01T00:00:00Z',
  }
}

const trades: Trade[] = [
  tradeStub(SWING_ID, 't1'),
  tradeStub(SWING_ID, 't2'),
  tradeStub(VALUE_ID, 't3'),
]

function Interactive({ initial }: { initial: StrategyFilter }) {
  const [value, setValue] = useState<StrategyFilter>(initial)
  return (
    <StrategyFilterBar
      trades={trades}
      strategies={strategies}
      value={value}
      onChange={setValue}
    />
  )
}

const meta = {
  title: 'Trades/StrategyFilterBar',
  parameters: { layout: 'padded' },
} satisfies Meta

export default meta
type Story = StoryObj<typeof meta>

export const All: Story = {
  render: () => <Interactive initial="all" />,
}

export const SwingActive: Story = {
  render: () => <Interactive initial={SWING_ID} />,
}
