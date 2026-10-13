import type { Meta, StoryObj } from '@storybook/react-vite'

import { StockPageHoldings } from '#components/stock-page/stock-page-holdings'

const meta = {
  title: 'StockPage/StockPageHoldings',
  component: StockPageHoldings,
  args: {
    position: {
      symbol: 'demo-code',
      qty: 100,
      avg_cost: 3200,
      cost_basis: 320000,
      realized_pnl: 4800,
    },
  },
} satisfies Meta<typeof StockPageHoldings>

export default meta
type Story = StoryObj<typeof meta>

export const WithPosition: Story = {
  name: '保有数量と損益を 4 つの指標で表示する。',
}

export const Loading: Story = {
  name: '保有情報の読み込み中を表示する。',
  args: { position: null, isLoading: true },
}

export const Error: Story = {
  name: '保有情報を取得できなかった状態を表示する。',
  args: { position: null, hasError: true },
}
