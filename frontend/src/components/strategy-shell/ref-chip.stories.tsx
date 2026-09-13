import type { Meta, StoryObj } from '@storybook/react-vite'

import { RefChip } from '#components/strategy-shell/ref-chip'
import { RefResolveQueryDecorator } from '#storybook/ref-resolve-mock'

const meta = {
  title: 'StrategyShell/RefChip',
  component: RefChip,
  decorators: [
    (Story) => (
      <RefResolveQueryDecorator>
        <Story />
      </RefResolveQueryDecorator>
    ),
  ],
} satisfies Meta<typeof RefChip>

export default meta
type Story = StoryObj<typeof meta>

export const Stock: Story = {
  args: { token: 'stock:7203' },
}

export const Indicator: Story = {
  args: { token: 'indicator:USDJPY' },
}

export const Sector: Story = {
  args: { token: 'sector:半導体' },
}

export const Theme: Story = {
  args: { token: 'theme:円安' },
}

export const Pill: Story = {
  args: { token: 'stock:3436', pill: true },
}

export const Unknown: Story = {
  args: { token: 'stock:9999' },
}
