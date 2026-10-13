import type { Meta, StoryObj } from '@storybook/react-vite'

import { AllocationRatios } from '#components/portfolio/allocation-ratios'

const meta = {
  title: 'Portfolio/AllocationRatios',
  component: AllocationRatios,
} satisfies Meta<typeof AllocationRatios>

export default meta
type Story = StoryObj<typeof meta>

export const Available: Story = {
  name: 'shows the cash and investment allocation ratios.',
  args: { cashRatio: 42.5, investedRatio: 57.5 },
}

export const PartialMarketValue: Story = {
  name: 'uses priced holdings and cash when another holding has no market value.',
  args: { cashRatio: 74.3, investedRatio: 25.7 },
}

export const Unavailable: Story = {
  name: 'marks both allocation ratios as unavailable.',
  args: { cashRatio: null, investedRatio: null },
}
