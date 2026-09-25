import type { Meta, StoryObj } from '@storybook/react-vite'
import { fn } from 'storybook/test'

import { ChartMarketDepthPanel } from '#components/chart-market-depth-panel'

const meta = {
  title: 'Components/ChartMarketDepthPanel',
  component: ChartMarketDepthPanel,
  decorators: [
    (Story) => (
      <div style={{ height: '400px' }}>
        <Story />
      </div>
    ),
  ],
  args: {
    onToggle: fn(),
  },
} satisfies Meta<typeof ChartMarketDepthPanel>

export default meta
type Story = StoryObj<typeof meta>

export const Open: Story = {
  name: 'shows the open market depth panel for a selected instrument.',
  args: {
    instrumentId: '7203',
    isOpen: true,
  },
}
