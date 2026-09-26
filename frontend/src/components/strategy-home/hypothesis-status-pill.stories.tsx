import type { Meta, StoryObj } from '@storybook/react-vite'

import { HypothesisStatusPill } from '#components/strategy-home/hypothesis-status-pill'

const meta = {
  title: 'StrategyHome/HypothesisStatusPill',
  component: HypothesisStatusPill,
} satisfies Meta<typeof HypothesisStatusPill>

export default meta
type Story = StoryObj<typeof meta>

export const Unverified: Story = {
  name: 'marks a hypothesis as unverified.',
  args: { status: 'unverified' },
}
export const Supported: Story = {
  name: 'marks a hypothesis as supported.',
  args: { status: 'supported' },
}
export const Refuted: Story = {
  name: 'marks a hypothesis as refuted.',
  args: { status: 'refuted' },
}
export const Obsolete: Story = {
  name: 'marks a hypothesis as obsolete.',
  args: { status: 'obsolete' },
}
