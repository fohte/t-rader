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
  name: 'shows market value, unrealized return, cash, and realized profit.',
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
      {
        label: '現金',
        value: '¥410,000',
        sub: '36.6% 比率',
      },
      {
        label: '実現損益 (累計)',
        value: '+¥12,300',
        cls: 'text-up',
      },
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
      { label: '現金', value: '¥410,000', sub: '— 比率' },
      { label: '実現損益 (累計)', value: '+¥12,300', cls: 'text-up' },
    ],
  },
}

export const Performance: Story = {
  name: 'summarizes realized performance and trading activity.',
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
