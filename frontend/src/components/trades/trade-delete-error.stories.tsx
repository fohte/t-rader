import type { Meta, StoryObj } from '@storybook/react-vite'

import { TradeDeleteError } from '#components/trades/trade-delete-error'

const meta = {
  title: 'Trades/TradeDeleteError',
  component: TradeDeleteError,
} satisfies Meta<typeof TradeDeleteError>

export default meta
type Story = StoryObj<typeof meta>

export const Visible: Story = {
  name: 'shows an error after a trade cannot be deleted.',
  args: { message: '取引を削除できませんでした。' },
}
