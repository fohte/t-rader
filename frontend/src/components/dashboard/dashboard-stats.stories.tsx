import type { Meta, StoryObj } from '@storybook/react-vite'

import { DashboardStats } from '#components/dashboard/dashboard-stats'

const meta = {
  title: 'Dashboard/DashboardStats',
  component: DashboardStats,
  parameters: { layout: 'padded' },
  args: { isError: false },
} satisfies Meta<typeof DashboardStats>

export default meta
type Story = StoryObj<typeof meta>

export const Portfolio: Story = {
  name: 'shows the four portfolio totals.',
  args: {
    stats: [
      { label: '総資産', value: '¥1,234,000' },
      { label: '評価額 (株式・簿価)', value: '¥834,000' },
      { label: '現金', value: '¥400,000' },
      { label: '実現損益 (累計)', value: '+¥12,300', cls: 'text-up' },
    ],
  },
}

export const Loading: Story = {
  name: 'shows a placeholder while portfolio totals are loading.',
  args: { stats: null },
}

export const Error: Story = {
  name: 'shows an error message when portfolio totals cannot be loaded.',
  args: { stats: null, isError: true },
}
