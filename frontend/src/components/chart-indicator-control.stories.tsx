import type { Meta, StoryObj } from '@storybook/react-vite'

import { ChartIndicatorControl } from '#components/chart-indicator-control'
import type { ChartIndicatorConfig } from '#lib/chart-indicator-settings'

const indicators: ChartIndicatorConfig[] = [
  { id: 'indicator-sma', kind: 'sma', period: 25, color: '#f97316' },
  {
    id: 'indicator-bollinger',
    kind: 'bollinger',
    period: 20,
    deviation: 2,
    color: '#38bdf8',
  },
]

const meta = {
  title: 'Components/ChartIndicatorControl',
  component: ChartIndicatorControl,
  args: {
    indicators: [],
    onAdd: () => {},
    onUpdatePeriod: () => {},
    onUpdateDeviation: () => {},
    onRemove: () => {},
    open: true,
    onOpenChange: () => {},
  },
} satisfies Meta<typeof ChartIndicatorControl>

export default meta
type Story = StoryObj<typeof meta>

export const EmptyPopover: Story = {
  name: 'the popover offers indicators when none are selected.',
  args: { layout: 'popover' },
}

export const ConfiguredPopover: Story = {
  name: 'the popover shows editable moving averages and bands.',
  args: { indicators, layout: 'popover' },
}

export const ConfiguredMobileSheet: Story = {
  name: 'the mobile sheet shows editable indicator settings.',
  args: { indicators, layout: 'sheet' },
}
