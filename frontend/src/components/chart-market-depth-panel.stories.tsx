import type { Meta, StoryObj } from '@storybook/react-vite'
import { expect, fn, within } from 'storybook/test'

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
  args: {
    instrumentId: '7203',
    isOpen: true,
  },
}

export const Closed: Story = {
  args: {
    instrumentId: '7203',
    isOpen: false,
  },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement)
    await expect(canvas.queryByText('板情報・歩み値')).not.toBeInTheDocument()
  },
}
