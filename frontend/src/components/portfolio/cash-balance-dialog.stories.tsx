import type { Meta, StoryObj } from '@storybook/react-vite'

import { CashBalanceDialog } from '#components/portfolio/cash-balance-dialog'

const meta = {
  title: 'Portfolio/CashBalanceDialog',
  component: CashBalanceDialog,
  parameters: { layout: 'centered' },
  args: {
    onOpenChange: () => undefined,
    onSave: () => undefined,
  },
} satisfies Meta<typeof CashBalanceDialog>

export default meta
type Story = StoryObj<typeof meta>

export const Open: Story = {
  name: 'shows the cash balance dialog with an existing balance.',
  args: {
    open: true,
    initial: 1000000,
  },
}

export const EmptyInitial: Story = {
  name: 'shows the cash balance dialog with no balance recorded.',
  args: {
    open: true,
    initial: 0,
  },
}
