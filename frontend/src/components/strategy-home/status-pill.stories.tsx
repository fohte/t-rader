import type { Meta, StoryObj } from '@storybook/react-vite'

import { StatusPill } from '#components/strategy-home/status-pill'

const meta = {
  title: 'StrategyHome/StatusPill',
  component: StatusPill,
} satisfies Meta<typeof StatusPill>

export default meta
type Story = StoryObj<typeof meta>

export const Approved: Story = { args: { status: 'approved' } }
export const Unread: Story = { args: { status: 'unread' } }
export const Rejected: Story = { args: { status: 'rejected' } }
