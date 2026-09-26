import type { Meta, StoryObj } from '@storybook/react-vite'

import { StatusPill } from '#components/strategy-home/status-pill'

const meta = {
  title: 'StrategyHome/StatusPill',
  component: StatusPill,
} satisfies Meta<typeof StatusPill>

export default meta
type Story = StoryObj<typeof meta>

export const Approved: Story = {
  name: 'shows an approved status.',
  args: { status: 'approved' },
}
export const Unread: Story = {
  name: 'shows an unread status.',
  args: { status: 'unread' },
}
export const Rejected: Story = {
  name: 'shows a rejected status.',
  args: { status: 'rejected' },
}
