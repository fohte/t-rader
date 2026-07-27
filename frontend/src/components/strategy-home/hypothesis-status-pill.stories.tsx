import type { Meta, StoryObj } from '@storybook/react-vite'

import { HypothesisStatusPill } from '#components/strategy-home/hypothesis-status-pill'

const meta = {
  title: 'StrategyHome/HypothesisStatusPill',
  component: HypothesisStatusPill,
} satisfies Meta<typeof HypothesisStatusPill>

export default meta
type Story = StoryObj<typeof meta>

export const Unverified: Story = { args: { status: 'unverified' } }
export const Supported: Story = { args: { status: 'supported' } }
export const Refuted: Story = { args: { status: 'refuted' } }
export const Obsolete: Story = { args: { status: 'obsolete' } }
