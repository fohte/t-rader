import type { Meta, StoryObj } from '@storybook/react-vite'
import { fn } from 'storybook/test'

import { PortfolioHistoryChart } from '#components/portfolio/portfolio-history-chart'
import type { PortfolioHistoryPoint } from '#lib/portfolio-history'

const history: PortfolioHistoryPoint[] = [
  {
    date: '2026-08-03',
    total_assets: 1_000_000,
    stock_market_value: 600_000,
    cash: 400_000,
    investable_amount: 1_000_000,
  },
  {
    date: '2026-08-07',
    total_assets: 1_018_000,
    stock_market_value: 618_000,
    cash: 400_000,
    investable_amount: 1_000_000,
  },
  {
    date: '2026-08-10',
    total_assets: 1_012_000,
    stock_market_value: 612_000,
    cash: 400_000,
    investable_amount: 1_000_000,
  },
  {
    date: '2026-08-14',
    total_assets: 1_025_000,
    stock_market_value: 625_000,
    cash: 400_000,
    investable_amount: 1_000_000,
  },
  {
    date: '2026-08-17',
    total_assets: 1_021_000,
    stock_market_value: 621_000,
    cash: 400_000,
    investable_amount: 1_000_000,
  },
  {
    date: '2026-08-21',
    total_assets: 1_032_000,
    stock_market_value: 632_000,
    cash: 400_000,
    investable_amount: 1_000_000,
  },
  {
    date: '2026-08-24',
    total_assets: 1_045_000,
    stock_market_value: 645_000,
    cash: 400_000,
    investable_amount: 1_000_000,
  },
  {
    date: '2026-08-28',
    total_assets: 1_038_000,
    stock_market_value: 638_000,
    cash: 400_000,
    investable_amount: 1_000_000,
  },
]

const meta = {
  title: 'Components/Portfolio/PortfolioHistoryChart',
  component: PortfolioHistoryChart,
  args: {
    data: history,
    period: 'month',
    status: 'ready',
    onPeriodChange: fn(),
  },
} satisfies Meta<typeof PortfolioHistoryChart>

export default meta
type Story = StoryObj<typeof meta>

export const Daily: Story = {
  name: 'shows the portfolio history for each day.',
  args: { period: 'day' },
}

export const Weekly: Story = {
  name: 'shows the portfolio history for each week.',
  args: { period: 'week' },
}

export const Tooltip: Story = {
  name: 'shows details for a hovered weekly point.',
  args: { period: 'week', initialHoveredDate: '2026-08-14' },
}

export const Monthly: Story = {
  name: 'shows the portfolio history for each month.',
  args: { period: 'month' },
}

export const Yearly: Story = {
  name: 'shows the portfolio history for each year.',
  args: { period: 'year' },
}

export const Loading: Story = {
  name: 'keeps the section layout while history is loading.',
  args: { data: [], status: 'loading' },
}

export const Empty: Story = {
  name: 'explains that history is empty before the first trade.',
  args: { data: [], status: 'empty' },
}

export const Error: Story = {
  name: 'shows a message when portfolio history cannot be loaded.',
  args: {
    data: [],
    status: 'error',
    errorMessage: '資産推移を読み込めませんでした',
  },
}
