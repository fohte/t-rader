import type { Meta, StoryObj } from '@storybook/react-vite'

import { IndicatorObservationChart } from '#components/indicator-observation-chart'
import type { IndicatorObservation } from '#lib/indicator-observation'

const observations: IndicatorObservation[] = [
  { date: '2025-04-01', value: 101.2 },
  { date: '2025-04-02', value: 102.4 },
  { date: '2025-04-03', value: 100.8 },
  { date: '2025-04-04', value: 103.1 },
  { date: '2025-04-07', value: 104.6 },
]

const meta = {
  title: 'Indicators/IndicatorObservationChart',
  component: IndicatorObservationChart,
  args: {
    observations,
    period: '1y',
    onPeriodChange: () => {},
    isPending: false,
    isError: false,
  },
  decorators: [
    (Story) => (
      <div className="w-full max-w-2xl bg-background p-5 text-foreground">
        <Story />
      </div>
    ),
  ],
} satisfies Meta<typeof IndicatorObservationChart>

export default meta
type Story = StoryObj<typeof meta>

export const Sample: Story = {
  name: 'shows a daily indicator series with the one year period selected.',
}

export const Empty: Story = {
  name: 'shows an empty state when the selected period has no observations.',
  args: { observations: [] },
}

export const Loading: Story = {
  name: 'shows loading feedback while observations are being fetched.',
  args: { observations: [], isPending: true },
}

export const Error: Story = {
  name: 'shows an error when observations cannot be loaded.',
  args: { observations: [], isError: true },
}
