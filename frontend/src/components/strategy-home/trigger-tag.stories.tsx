import type { Meta, StoryObj } from '@storybook/react-vite'

import { TriggerTag } from '#components/strategy-home/trigger-tag'

const meta = {
  title: 'StrategyHome/TriggerTag',
  component: TriggerTag,
} satisfies Meta<typeof TriggerTag>

export default meta
type Story = StoryObj<typeof meta>

export const Cron: Story = {
  name: 'shows a trigger scheduled to run every day.',
  args: { trigger: 'cron', label: '毎日 07:00 JST' },
}
export const Hook: Story = {
  name: 'shows a trigger tied to an earnings announcement.',
  args: { trigger: 'hook', label: '決算発表 hook' },
}
export const OnDemand: Story = {
  name: 'shows a trigger started from chat.',
  args: { trigger: 'on-demand', label: 'チャットから' },
}
export const Manual: Story = {
  name: 'shows a trigger started manually.',
  args: { trigger: 'manual', label: '手動' },
}
