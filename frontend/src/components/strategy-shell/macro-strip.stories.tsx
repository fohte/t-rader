import type { Meta, StoryObj } from '@storybook/react-vite'

import { MacroStripView } from '@/components/strategy-shell/macro-strip'

const meta = {
  title: 'StrategyShell/MacroStrip',
  component: MacroStripView,
  parameters: { layout: 'fullscreen' },
} satisfies Meta<typeof MacroStripView>

export default meta
type Story = StoryObj<typeof meta>

const sampleTicks = [
  {
    symbol: '日経225',
    value: '38420.55',
    pct: -0.62,
    fetched_at: '2026-06-25T06:00:00Z',
  },
  {
    symbol: 'TOPIX',
    value: '2711.30',
    pct: -0.41,
    fetched_at: '2026-06-25T06:00:00Z',
  },
  {
    symbol: 'USD/JPY',
    value: '157.84',
    pct: 0.38,
    fetched_at: '2026-06-25T06:00:00Z',
  },
  {
    symbol: 'VIX',
    value: '18.92',
    pct: 4.71,
    fetched_at: '2026-06-25T06:00:00Z',
  },
]

export const Fresh: Story = {
  args: { ticks: sampleTicks, staleSince: null, isPending: false },
}

export const Loading: Story = {
  args: { ticks: null, staleSince: null, isPending: true },
}

export const Stale: Story = {
  args: {
    ticks: sampleTicks,
    staleSince: '2026-06-25T01:00:00Z',
    isPending: false,
  },
}

export const NotAvailable: Story = {
  args: {
    ticks: null,
    staleSince: '2026-06-23T00:00:00Z',
    isPending: false,
  },
}
