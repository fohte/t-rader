import type { Meta, StoryObj } from '@storybook/react-vite'

import { StrategyListCount } from '#components/strategy-home/strategy-list-count'

const meta = {
  title: 'StrategyHome/StrategyListCount',
  component: StrategyListCount,
  args: { count: 3 },
} satisfies Meta<typeof StrategyListCount>

export default meta
type Story = StoryObj<typeof meta>

export const WithStrategies: Story = {
  name: 'shows the number of available strategies.',
}

export const Empty: Story = {
  name: 'shows zero when there are no strategies.',
  args: { count: 0 },
}
