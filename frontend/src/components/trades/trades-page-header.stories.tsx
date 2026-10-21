import type { Meta, StoryObj } from '@storybook/react-vite'

import { TradesPageHeader } from '#components/trades/trades-page-header'

const meta = {
  title: 'Trades/TradesPageHeader',
  component: TradesPageHeader,
  parameters: { layout: 'padded' },
  args: {
    onImport: () => {},
    onAdd: () => {},
  },
} satisfies Meta<typeof TradesPageHeader>

export default meta
type Story = StoryObj<typeof meta>

export const Default: Story = {
  name: 'shows the trade page title with import and add actions.',
}
