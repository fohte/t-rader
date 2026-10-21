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
  { id: 'indicator-rsi', kind: 'rsi', period: 14, color: '#34d399' },
  {
    id: 'indicator-macd',
    kind: 'macd',
    fastPeriod: 12,
    slowPeriod: 26,
    signalPeriod: 9,
    color: '#c084fc',
  },
]

const meta = {
  title: 'Components/ChartIndicatorControl',
  component: ChartIndicatorControl,
  decorators: [
    (Story) => (
      <div className="grid min-h-screen place-items-center">
        <Story />
      </div>
    ),
  ],
  args: {
    indicators: [],
    onAdd: () => {},
    onUpdatePeriod: () => {},
    onUpdateMacdPeriod: () => {},
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
  name: 'the popover shows editable settings for every indicator type.',
  args: { indicators, layout: 'popover' },
}

export const ConfiguredDialog: Story = {
  name: 'the centered dialog shows editable settings for every indicator type.',
  args: { indicators, layout: 'dialog' },
}
