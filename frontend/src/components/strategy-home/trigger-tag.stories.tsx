import type { Meta, StoryObj } from '@storybook/react-vite'

import { TriggerTag } from '#components/strategy-home/trigger-tag'

const meta = {
  title: 'StrategyHome/TriggerTag',
  component: TriggerTag,
} satisfies Meta<typeof TriggerTag>

export default meta
type Story = StoryObj<typeof meta>

export const Cron: Story = {
  args: { trigger: 'cron', label: '毎日 07:00 JST' },
}
export const Hook: Story = {
  args: { trigger: 'hook', label: '決算発表 hook' },
}
export const OnDemand: Story = {
  args: { trigger: 'on-demand', label: 'チャットから' },
}
export const Manual: Story = {
  args: { trigger: 'manual', label: '手動' },
}
