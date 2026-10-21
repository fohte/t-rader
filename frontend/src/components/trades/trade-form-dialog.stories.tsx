import type { Meta, StoryObj } from '@storybook/react-vite'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'

import { TradeFormDialog } from '#components/trades/trade-form-dialog'
import type { components } from '#lib/api/schema.gen'

type Trade = components['schemas']['Trade']
type Strategy = components['schemas']['Strategy']
type Stock = components['schemas']['Stock']

const strategyId = '00000000-0000-0000-0000-000000000001'
const strategy: Strategy = {
  id: strategyId,
  name: '架空戦略 A',
  description: null,
  sort_order: 0,
  created_at: '2099-01-01T00:00:00Z',
  updated_at: '2099-01-01T00:00:00Z',
}
const stock: Stock = {
  id: 'DEMO-A',
  name: '架空商事',
  market: null,
  sector_id: null,
  created_at: '2099-01-01T00:00:00Z',
  updated_at: '2099-01-01T00:00:00Z',
}
const trade: Trade = {
  id: '00000000-0000-0000-0000-000000000010',
  strategy_id: strategyId,
  symbol: stock.id,
  side: 'buy',
  qty: 20,
  price: 1200,
  fee: 150,
  date: '2099-01-03',
  source: 'manual',
  note: '架空の取引メモ',
  created_at: '2099-01-03T00:00:00Z',
  updated_at: '2099-01-03T00:00:00Z',
}

const queryClient = new QueryClient()

const meta = {
  title: 'Trades/TradeFormDialog',
  component: TradeFormDialog,
  parameters: { layout: 'centered' },
  decorators: [
    (Story) => (
      <QueryClientProvider client={queryClient}>
        <Story />
      </QueryClientProvider>
    ),
  ],
  args: {
    open: true,
    onOpenChange: () => {},
    initial: trade,
    strategies: [strategy],
    stocks: [stock],
  },
} satisfies Meta<typeof TradeFormDialog>

export default meta
type Story = StoryObj<typeof meta>

export const EditingTrade: Story = {
  name: 'shows an existing trade in the edit form.',
}

export const AddingTrade: Story = {
  name: 'shows the fields for adding a new trade.',
  args: { initial: null, defaultDate: '2099-01-03' },
}
