import type { Meta, StoryObj } from '@storybook/react-vite'
import { RouterProvider } from '@tanstack/react-router'
import type { ComponentType } from 'react'

import { StockPageFinancials } from '#components/stock-page/stock-page-financials'
import type { components } from '#lib/api/schema.gen'
import { createStoryRouter } from '#storybook/story-router'

type FinancialSummary = components['schemas']['StockFinancialSummaryResponse']
type GroupMembership = components['schemas']['StockGroupMembershipResponse']

const summaries: FinancialSummary[] = [
  {
    code: 'demo-code',
    disclosure_no: 'demo-disclosure-older',
    disclosure_date: '2030-03-20',
    report_group_key: 'demo-period-older',
    document_type: 'FYFinancialStatements_SampleA',
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
    document_type: '2QFinancialStatements_SampleB',
    current_period_type: '2Q',
    current_period_end: '2029-09-30',
    current_fiscal_year_end: '2030-03-31',
    sales: 410_000_000_000,
    operating_profit: 21_000_000_000,
    eps: 114,
  },
]

const periodSummaries: FinancialSummary[] = [
  {
    code: 'demo-code',
    disclosure_no: 'demo-period-disclosure-a',
    disclosure_date: '2030-11-10',
    report_group_key: 'demo-period-group-a',
    document_type: '2QFinancialStatements_SampleC',
    current_period_type: '2Q',
    current_period_end: '2030-09-30',
    sales: 410_000_000_000,
    operating_profit: 21_000_000_000,
    eps: 114,
  },
  {
    code: 'demo-code',
    disclosure_no: 'demo-period-disclosure-b',
    disclosure_date: '2030-11-10',
    report_group_key: 'demo-period-group-b',
    document_type: '2QFinancialStatements_SampleD',
    current_period_type: '2Q',
    current_period_end: '2030-09-30',
    sales: 420_000_000_000,
    operating_profit: 22_000_000_000,
    eps: 116,
  },
]

const groups: GroupMembership[] = [
  {
    axis: { key: 'demo-axis', name: 'デモ軸' },
    group: { key: 'demo-group', name: 'デモ群' },
  },
]

function withRouter(Story: ComponentType) {
  return (
    <RouterProvider
      router={createStoryRouter(
        () => (
          <Story />
        ),
        {
          paths: ['/groups/$axisKey/$groupKey'],
        },
      )}
    />
  )
}

const meta = {
  title: 'StockPage/StockPageFinancials',
  component: StockPageFinancials,
  decorators: [withRouter],
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

export const SelectedPeriodWithAllDisclosureCandidates: Story = {
  name: '指定した期に一致する全開示を開示日と書類種別付きで表示します。',
  args: {
    summaries: periodSummaries,
    selectedPeriod: { end: '2030-09-30', type: '2Q' },
  },
}

export const WithoutSummaries: Story = {
  name: '決算短信の要約がない状態を表示する。',
  args: {
    summaries: [],
    selectedPeriod: { end: '2030-09-30', type: '2Q' },
  },
}

export const Loading: Story = {
  name: '決算短信の要約を読み込んでいる状態を表示する。',
  args: { summaries: [], summariesState: 'loading' },
}

export const Error: Story = {
  name: '決算短信の要約を取得できなかった状態を表示する。',
  args: { summaries: [], summariesState: 'error' },
}
