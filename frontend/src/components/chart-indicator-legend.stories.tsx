import type { Meta, StoryObj } from '@storybook/react-vite'

import { ChartIndicatorLegend } from '#components/chart-indicator-legend'
import type { ChartIndicatorConfig } from '#lib/chart-indicator-settings'

const indicators: ChartIndicatorConfig[] = [
  { id: 'indicator-sma', kind: 'sma', period: 25, color: '#f97316' },
  { id: 'indicator-rsi', kind: 'rsi', period: 14, color: '#38bdf8' },
  {
    id: 'indicator-macd',
    kind: 'macd',
    fastPeriod: 12,
    slowPeriod: 26,
    signalPeriod: 9,
    color: '#34d399',
  },
]

const meta = {
  title: 'Components/ChartIndicatorLegend',
  component: ChartIndicatorLegend,
  decorators: [
    (Story) => (
      <div className="relative h-full w-full" style={{ height: '380px' }}>
        <Story />
      </div>
    ),
  ],
  args: { indicators, paneTops: [0, 196, 276] },
} satisfies Meta<typeof ChartIndicatorLegend>

export default meta
type Story = StoryObj<typeof meta>

export const PriceAndLowerPanes: Story = {
  name: '価格と RSI・MACD をそれぞれの段に表示する。',
}
