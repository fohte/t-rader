import type { Meta, StoryObj } from '@storybook/react-vite'

import { ChartIndicatorLegend } from '#components/chart-indicator-legend'
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
  title: 'Components/ChartIndicatorLegend',
  component: ChartIndicatorLegend,
} satisfies Meta<typeof ChartIndicatorLegend>

export default meta
type Story = StoryObj<typeof meta>

export const SeveralIndicators: Story = {
  name: 'the chart legend identifies each visible overlay.',
  args: { indicators },
  render: (args) => (
    <div className="relative h-24 border border-border">
      <ChartIndicatorLegend {...args} />
    </div>
  ),
}
