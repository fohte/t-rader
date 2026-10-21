import type { Meta, StoryObj } from '@storybook/react-vite'

import { CreateStrategyCard } from '#components/strategy-home/create-strategy-card'

const meta = {
  title: 'StrategyHome/CreateStrategyCard',
  component: CreateStrategyCard,
  args: { onClick: () => {} },
} satisfies Meta<typeof CreateStrategyCard>

export default meta
type Story = StoryObj<typeof meta>

export const Default: Story = {
  name: 'offers the action to create a strategy.',
}
