import type { Meta, StoryObj } from '@storybook/react-vite'

import { StrategyListLoading } from '#components/strategy-home/strategy-list-loading'

const meta = {
  title: 'StrategyHome/StrategyListLoading',
  component: StrategyListLoading,
} satisfies Meta<typeof StrategyListLoading>

export default meta
type Story = StoryObj<typeof meta>

export const Loading: Story = {
  name: 'shows the strategy cards while the list is loading.',
}
