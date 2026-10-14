import type { Meta, StoryObj } from '@storybook/react-vite'

import { StrategyListIntroduction } from '#components/strategy-home/strategy-list-introduction'

const meta = {
  title: 'StrategyHome/StrategyListIntroduction',
  component: StrategyListIntroduction,
  parameters: { layout: 'padded' },
} satisfies Meta<typeof StrategyListIntroduction>

export default meta
type Story = StoryObj<typeof meta>

export const Default: Story = {
  name: 'the header explains what a strategy represents.',
}
