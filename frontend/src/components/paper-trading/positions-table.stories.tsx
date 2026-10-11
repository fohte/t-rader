import type { Meta, StoryObj } from '@storybook/react-vite'

import { PaperTradePositionsTable } from '#components/paper-trading/positions-table'
import type { components } from '#lib/api/schema.gen'

type PaperTradePosition = components['schemas']['PaperTradePosition']

const positions: PaperTradePosition[] = [
  {
    stock_id: 'SIM-002',
    qty: 50,
    avg_cost_jpy: 8_900,
    current_price_jpy: 8_800,
    market_value_jpy: 440_000,
    unrealized_pnl_jpy: -5_000,
  },
  {
    stock_id: 'SIM-001',
    qty: 100,
    avg_cost_jpy: 8_500,
    current_price_jpy: 8_750,
    market_value_jpy: 875_000,
    unrealized_pnl_jpy: 25_000,
  },
]

const stockNames: Record<string, string> = {
  'SIM-001': '架空銘柄A',
  'SIM-002': '架空銘柄B',
}

const meta = {
  title: 'PaperTrading/PositionsTable',
  component: PaperTradePositionsTable,
  parameters: { layout: 'padded' },
  args: { positions, stockNames },
} satisfies Meta<typeof PaperTradePositionsTable>

export default meta
type Story = StoryObj<typeof meta>

export const WithPositions: Story = {
  name: '保有銘柄の取得単価と含み損益を表示する。',
}

export const Empty: Story = {
  name: '保有銘柄がないときに空状態を表示する。',
  args: { positions: [] },
}
