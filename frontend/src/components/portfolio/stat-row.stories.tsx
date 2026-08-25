import type { Meta, StoryObj } from '@storybook/react-vite'

import { StatRow } from '#components/portfolio/stat-row'

const meta = {
  title: 'Portfolio/StatRow',
  component: StatRow,
  parameters: { layout: 'padded' },
} satisfies Meta<typeof StatRow>

export default meta
type Story = StoryObj<typeof meta>

export const PortfolioOverview: Story = {
  args: {
    stats: [
      { label: '総資産', value: '¥4,820,000' },
      { label: '評価額 (株式・簿価)', value: '¥3,250,000' },
      {
        label: '現金',
        value: '¥1,570,000',
        sub: '32.6% 比率',
      },
      {
        label: '実現損益 (累計)',
        value: '+¥124,500',
        cls: 'text-up',
      },
      { label: '保有銘柄', value: '5' },
    ],
  },
}

export const Performance: Story = {
  args: {
    stats: [
      {
        label: '実現損益',
        value: '−¥38,200',
        cls: 'text-down',
      },
      {
        label: '手数料',
        value: '¥4,820',
        cls: 'text-muted-foreground-strong',
      },
      { label: '決済回数', value: '12' },
      { label: 'トレード件数', value: '28' },
      { label: '保有銘柄', value: '3' },
    ],
  },
}
