import type { Meta, StoryObj } from '@storybook/react-vite'

import { TradeStats } from '#components/trades/trade-stats'

const meta = {
  title: 'Trades/TradeStats',
  component: TradeStats,
  parameters: { layout: 'padded' },
} satisfies Meta<typeof TradeStats>

export default meta
type Story = StoryObj<typeof meta>

export const Default: Story = {
  name: 'summarizes profitable trading with fees and open positions.',
  args: {
    realizedPnl: 124500,
    feesTotal: 8920,
    tradeCount: 42,
    openPositions: 5,
  },
}

export const Loss: Story = {
  name: 'shows a net loss alongside fees and open positions.',
  args: {
    realizedPnl: -38200,
    feesTotal: 12400,
    tradeCount: 28,
    openPositions: 3,
  },
}

export const Empty: Story = {
  name: 'shows zeroed trading stats before any trades are recorded.',
  args: {
    realizedPnl: 0,
    feesTotal: 0,
    tradeCount: 0,
    openPositions: 0,
  },
}
