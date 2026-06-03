import type { Meta, StoryObj } from '@storybook/react-vite'

import { MacroStrip } from '@/components/strategy-shell/macro-strip'

const meta = {
  title: 'StrategyShell/MacroStrip',
  component: MacroStrip,
  parameters: { layout: 'fullscreen' },
} satisfies Meta<typeof MacroStrip>

export default meta
type Story = StoryObj<typeof meta>

export const Default: Story = {}

export const Custom: Story = {
  args: {
    ticks: [
      { name: 'BTC', value: '67,840', pct: 1.23 },
      { name: 'ETH', value: '3,512', pct: -0.41 },
      { name: 'GOLD', value: '2,381', pct: 0.07 },
    ],
  },
}
