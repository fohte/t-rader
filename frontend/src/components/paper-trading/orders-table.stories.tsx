import type { Meta, StoryObj } from '@storybook/react-vite'
import { RouterProvider } from '@tanstack/react-router'

import { PaperTradeOrdersTable } from '#components/paper-trading/orders-table'
import type { components } from '#lib/api/schema.gen'
import { createStoryRouter } from '#storybook/story-router'

type PaperTradeOrder = components['schemas']['PaperTradeOrder']

const orders: PaperTradeOrder[] = [
  {
    order_id: '00000000-0000-4000-8000-000000000302',
    stock_id: 'SIM-002',
    side: 'sell',
    qty: 50,
    note_id: '00000000-0000-4000-8000-000000000402',
    note_version_id: '00000000-0000-4000-8000-000000000502',
    ordered_at: '2026-06-18T02:15:00Z',
    outcome: 'rejected',
    fill_date: null,
    fill_price_jpy: null,
    reject_reason: '保有数量を超えています',
  },
  {
    order_id: '00000000-0000-4000-8000-000000000301',
    stock_id: 'SIM-001',
    side: 'buy',
    qty: 100,
    note_id: '00000000-0000-4000-8000-000000000401',
    note_version_id: '00000000-0000-4000-8000-000000000501',
    ordered_at: '2026-06-20T01:30:00Z',
    outcome: 'filled',
    fill_date: '2026-06-20',
    fill_price_jpy: 8_500,
    reject_reason: null,
  },
  {
    order_id: '00000000-0000-4000-8000-000000000303',
    stock_id: 'SIM-002',
    side: 'buy',
    qty: 25,
    note_id: '00000000-0000-4000-8000-000000000403',
    note_version_id: '00000000-0000-4000-8000-000000000503',
    ordered_at: '2026-06-17T03:45:00Z',
    outcome: null,
    fill_date: null,
    fill_price_jpy: null,
    reject_reason: null,
  },
]

const stockNames: Record<string, string> = {
  'SIM-001': '架空銘柄A',
  'SIM-002': '架空銘柄B',
}

const meta = {
  title: 'PaperTrading/OrdersTable',
  component: PaperTradeOrdersTable,
  parameters: { layout: 'padded' },
  args: {
    orders,
    stockNames,
  },
  decorators: [
    (Story) => (
      <RouterProvider
        router={createStoryRouter(
          () => (
            <Story />
          ),
          { paths: ['/notes/$noteId'] },
        )}
      />
    ),
  ],
} satisfies Meta<typeof PaperTradeOrdersTable>

export default meta
type Story = StoryObj<typeof meta>

export const MixedOutcomes: Story = {
  name: '約定・却下・未約定の注文と根拠ノートを表示する。',
}

export const Empty: Story = {
  name: '注文がないときに空状態を表示する。',
  args: { orders: [] },
}
