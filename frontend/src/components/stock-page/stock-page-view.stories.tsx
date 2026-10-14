import type { Meta, StoryObj } from '@storybook/react-vite'
import { RouterProvider } from '@tanstack/react-router'

import { StockPageView } from '#components/stock-page/stock-page-view'
import type { components } from '#lib/api/schema.gen'
import { createStoryRouter } from '#storybook/story-router'

type Stock = components['schemas']['Stock']
type Position = components['schemas']['PositionSummary']
type Bar = components['schemas']['Bar']
type Annotation = components['schemas']['Annotation']
type Trade = components['schemas']['TradeListItem']
type GroupMembership = components['schemas']['StockGroupMembershipResponse']
type NextEarnings = components['schemas']['StockNextEarningsResponse']
type Valuation = components['schemas']['StockValuationResponse']
type FinancialSummary = components['schemas']['StockFinancialSummaryResponse']

const stock: Stock = {
  id: 'demo-code',
  name: 'デモ商事',
  market: 'TSE',
  sector_id: null,
  product_category: null,
  created_at: '2030-04-01T00:00:00Z',
  updated_at: '2030-04-01T00:00:00Z',
}

const position: Position = {
  symbol: 'demo-code',
  qty: 100,
  avg_cost: 3200,
  cost_basis: 320000,
  realized_pnl: 4800,
}

const bars: Bar[] = [
  {
    instrument_id: 'demo-code',
    timeframe: '1d',
    timestamp: '2030-04-01T06:00:00Z',
    open: 3000,
    high: 3100,
    low: 2980,
    close: 3060,
    volume: 10000,
  },
  {
    instrument_id: 'demo-code',
    timeframe: '1d',
    timestamp: '2030-04-02T06:00:00Z',
    open: 3060,
    high: 3140,
    low: 3030,
    close: 3110,
    volume: 12000,
  },
  {
    instrument_id: 'demo-code',
    timeframe: '1d',
    timestamp: '2030-04-03T06:00:00Z',
    open: 3110,
    high: 3260,
    low: 3090,
    close: 3250,
    volume: 15000,
  },
]

const annotations: Annotation[] = [
  {
    id: 'demo-annotation',
    target_kind: 'stock',
    target_symbol: 'demo-code',
    timestamp: '2030-04-02T06:00:00Z',
    timestamp_start: null,
    price: 3110,
    text: '出来高を伴って直近高値を上抜けたデモ用メモです。',
    linked_note_id: null,
    status: 'unread',
    created_by_kind: 'llm',
    execution_step_id: null,
    execution_task_id: null,
    created_at: '2030-04-02T06:00:00Z',
    updated_at: '2030-04-02T06:00:00Z',
  },
]

const trades: Trade[] = [
  {
    id: 'demo-trade',
    strategy_id: '00000000-0000-0000-0000-000000000001',
    symbol: 'demo-code',
    side: 'buy',
    qty: 100,
    price: 3200,
    fee: 0,
    date: '2030-04-01',
    source: 'manual',
    note: null,
    note_count: 0,
    created_at: '2030-04-01T06:00:00Z',
    updated_at: '2030-04-01T06:00:00Z',
  },
]

const groups: GroupMembership[] = [
  {
    axis: { key: 'demo-axis', name: 'デモ軸' },
    group: { key: 'demo-group', name: 'デモ群' },
  },
]

const nextEarnings: NextEarnings = {
  event_date: '2030-04-14',
  title: '決算発表予定',
  event_at: null,
  fiscal_period: 'デモ事業年度 第 2 四半期',
  time_of_day: '午後',
}

const valuation: Valuation = {
  code: 'demo-code',
  date: '2030-04-09',
  per: 16.2,
  fwd_per: 14.2,
  pbr: 1.3,
  roe: 9.1,
  fwd_roe: null,
  eps: 200,
  fwd_eps: 230,
  bps: 2500,
  mkt_cap: 123_000_000_000,
}

const financialSummaries: FinancialSummary[] = [
  {
    code: 'demo-code',
    disclosure_no: 'demo-disclosure',
    disclosure_date: '2030-04-03',
    report_group_key: 'demo-period',
    current_period_type: '2Q',
    current_period_end: '2029-09-30',
    current_fiscal_year_end: '2030-03-31',
    sales: 410_000_000_000,
    operating_profit: 21_000_000_000,
    eps: 114,
  },
]

const meta = {
  title: 'StockPage/StockPageView',
  component: StockPageView,
  parameters: { layout: 'fullscreen' },
  decorators: [
    (Story) => (
      <RouterProvider
        router={createStoryRouter(
          () => (
            <Story />
          ),
          {
            paths: ['/annotations/$annoId'],
          },
        )}
      />
    ),
  ],
  args: {
    stock,
    quote: {
      close: 3250,
      change: 50,
      changePercent: 1.56,
      timestamp: '2030-04-03T06:00:00Z',
    },
    quoteLoading: false,
    quoteError: false,
    bars,
    annotations,
    trades,
    groups,
    groupsState: 'ready',
    nextEarnings,
    nextEarningsState: 'ready',
    valuation,
    valuationState: 'ready',
    financialSummaries,
    financialSummariesState: 'ready',
    position,
    holdingsState: 'ready',
    timeframe: '1d',
    onTimeframeChange: () => undefined,
    selectedAnnotationId: 'demo-annotation',
    onSelectAnnotation: () => undefined,
    activeTab: 'annotations',
    onTabChange: () => undefined,
    currency: 'JPY',
    chartLoading: false,
    chartError: false,
    annotationState: 'ready',
    tradeState: 'ready',
  },
} satisfies Meta<typeof StockPageView>

export default meta
type Story = StoryObj<typeof meta>

export const DesktopPage: Story = {
  name: 'チャートと保有情報を並べた銘柄ページを表示する。',
}

export const WithoutPosition: Story = {
  name: '保有していない銘柄にも企業情報を表示する。',
  args: { position: null },
}

export const FinancialsTab: Story = {
  name: '財務タブに所属グループと決算短信の要約を表示する。',
  args: { activeTab: 'financials' },
}

export const DataErrors: Story = {
  name: '終値と保有情報を取得できなかった状態を表示する。',
  args: {
    quote: null,
    quoteError: true,
    position: null,
    holdingsState: 'error',
    groupsState: 'error',
    nextEarnings: null,
    nextEarningsState: 'error',
    valuation: null,
    valuationState: 'error',
    financialSummaries: [],
    financialSummariesState: 'error',
  },
}
