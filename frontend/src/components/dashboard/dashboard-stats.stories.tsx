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
  name: 'shows the five market value and cash summary metrics.',
  args: {
    stats: [
      { label: '総資産 (時価)', value: '¥1,120,000' },
      {
        label: '評価額 (株式・時価)',
        value: '¥710,000',
        sub: '簿価 ¥680,000',
      },
      {
        label: '含み損益',
        value: '+¥30,000',
        sub: '+4.4%',
        cls: 'text-up',
      },
      { label: '現金', value: '¥410,000', sub: '36.6% 比率' },
      { label: '実現損益 (累計)', value: '+¥12,300', cls: 'text-up' },
    ],
  },
}

export const MissingMarketValue: Story = {
  name: 'shows unavailable totals when a holding has no observed price.',
  args: {
    stats: [
      { label: '総資産 (時価)', value: '—' },
      {
        label: '評価額 (株式・時価)',
        value: '—',
        sub: '簿価 ¥680,000',
      },
      {
        label: '含み損益',
        value: '—',
        sub: '—',
        cls: 'text-muted-foreground-strong',
      },
      { label: '現金', value: '¥410,000', sub: '57.7% 既知額比' },
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
