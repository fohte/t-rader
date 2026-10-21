import type { Meta, StoryObj } from '@storybook/react-vite'

import {
  ImportSbiReview,
  type ImportSbiRow,
} from '#components/trades/import-sbi-review'
import type { components } from '#lib/api/schema.gen'

type Strategy = components['schemas']['Strategy']
const FIRST_STRATEGY_ID = '00000000-0000-0000-0000-000000000001'

function strategyStub(id: string, name: string, sortOrder: number): Strategy {
  return {
    id,
    name,
    description: null,
    sort_order: sortOrder,
    created_at: '2099-01-01T00:00:00Z',
    updated_at: '2099-01-01T00:00:00Z',
  }
}

const strategies: Strategy[] = [
  strategyStub(FIRST_STRATEGY_ID, '架空戦略 A', 0),
  strategyStub('00000000-0000-0000-0000-000000000002', '架空戦略 B', 1),
]

const rows: ImportSbiRow[] = [
  {
    row_index: 2,
    date: '2099-01-03',
    symbol: 'DEMO-A',
    stock_name: '架空商事',
    side: 'buy',
    qty: 20,
    price: 1200,
    fee: 150,
    is_duplicate: false,
    strategyId: FIRST_STRATEGY_ID,
    excluded: false,
  },
  {
    row_index: 3,
    date: '2099-01-02',
    symbol: 'DEMO-B',
    stock_name: '架空工業',
    side: 'sell',
    qty: 10,
    price: 1400,
    fee: 150,
    is_duplicate: true,
    strategyId: FIRST_STRATEGY_ID,
    excluded: true,
  },
]

const meta = {
  title: 'Trades/ImportSbiReview',
  component: ImportSbiReview,
  parameters: { layout: 'padded' },
  args: {
    rows,
    strategies,
    issues: [],
    bulkStrategy: FIRST_STRATEGY_ID,
    importableCount: 1,
    bulkSelectDefaultOpen: true,
    onBulkStrategyChange: () => {},
    onRowChange: () => {},
  },
} satisfies Meta<typeof ImportSbiReview>

export default meta
type Story = StoryObj<typeof meta>

export const StrategyMenuOpen: Story = {
  name: 'shows the import review with the strategy menu open.',
}
