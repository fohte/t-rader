import type { Meta, StoryObj } from '@storybook/react-vite'

import { StockPageFinancials } from '#components/stock-page/stock-page-financials'
import type { components } from '#lib/api/schema.gen'

type FinancialSummary = components['schemas']['StockFinancialSummaryResponse']
type GroupMembership = components['schemas']['StockGroupMembershipResponse']

const summaries: FinancialSummary[] = [
  {
    code: 'demo-code',
    disclosure_no: 'demo-disclosure-older',
    disclosure_date: '2030-03-20',
    report_group_key: 'demo-period-older',
    current_period_type: 'FY',
    current_period_end: '2030-03-31',
    current_fiscal_year_end: '2030-03-31',
    sales: 790_000_000_000,
    operating_profit: 38_000_000_000,
    eps: 205,
  },
  {
    code: 'demo-code',
    disclosure_no: 'demo-disclosure-newer',
    disclosure_date: '2030-04-03',
    report_group_key: 'demo-period-newer',
    current_period_type: '2Q',
    current_period_end: '2029-09-30',
    current_fiscal_year_end: '2030-03-31',
    sales: 410_000_000_000,
    operating_profit: 21_000_000_000,
    eps: 114,
  },
]

const groups: GroupMembership[] = [
  {
    axis: { key: 'demo-axis', name: 'デモ軸' },
    group: { key: 'demo-group', name: 'デモ群' },
  },
]

const meta = {
  title: 'StockPage/StockPageFinancials',
  component: StockPageFinancials,
  args: {
    summaries,
    summariesState: 'ready',
    groups,
    groupsState: 'ready',
  },
} satisfies Meta<typeof StockPageFinancials>

export default meta
type Story = StoryObj<typeof meta>

export const WithSummaries: Story = {
  name: '決算短信の要約を開示日の新しい順で表示する。',
}

export const WithoutSummaries: Story = {
  name: '決算短信の要約がない状態を表示する。',
  args: { summaries: [] },
}

export const Loading: Story = {
  name: '決算短信の要約を読み込んでいる状態を表示する。',
  args: { summaries: [], summariesState: 'loading' },
}

export const Error: Story = {
  name: '決算短信の要約を取得できなかった状態を表示する。',
  args: { summaries: [], summariesState: 'error' },
}
