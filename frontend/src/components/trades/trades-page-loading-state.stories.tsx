import type { Meta, StoryObj } from '@storybook/react-vite'

import { TradesPageLoadingState } from '#components/trades/trades-page-loading-state'

const meta = {
  title: 'Trades/TradesPageLoadingState',
  component: TradesPageLoadingState,
  parameters: { layout: 'padded' },
} satisfies Meta<typeof TradesPageLoadingState>

export default meta
type Story = StoryObj<typeof meta>

export const Summary: Story = {
  name: 'shows the loading placeholder for trade statistics.',
  args: { kind: 'summary' },
}

export const TradeTable: Story = {
  name: 'shows the loading placeholder for the trade table.',
  args: { kind: 'table' },
}
