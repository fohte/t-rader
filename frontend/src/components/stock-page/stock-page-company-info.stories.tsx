import type { Meta, StoryObj } from '@storybook/react-vite'

import { StockPageCompanyInfo } from '#components/stock-page/stock-page-company-info'
import type { components } from '#lib/api/schema.gen'

type Position = components['schemas']['PositionSummary']
type NextEarnings = components['schemas']['StockNextEarningsResponse']
type Valuation = components['schemas']['StockValuationResponse']

const position: Position = {
  symbol: 'demo-code',
  qty: 100,
  avg_cost: 3200,
  cost_basis: 320000,
  realized_pnl: 4800,
}

const earnings: NextEarnings = {
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

const meta = {
  title: 'StockPage/StockPageCompanyInfo',
  component: StockPageCompanyInfo,
  args: {
    position,
    holdingsState: 'ready',
    earnings,
    earningsState: 'ready',
    valuation,
    valuationState: 'ready',
  },
} satisfies Meta<typeof StockPageCompanyInfo>

export default meta
type Story = StoryObj<typeof meta>

export const CompanyOverview: Story = {
  name: '保有・次の決算・バリュエーションを表示する。',
}

export const MobileOverview: Story = {
  name: 'スマートフォンで企業情報を 2 列に表示する。',
  globals: { viewport: { value: 'mobileStock' } },
  parameters: {
    viewport: {
      options: {
        mobileStock: {
          name: 'Mobile stock page',
          styles: { width: '390px', height: '700px' },
          type: 'mobile',
        },
      },
    },
    screenshot: {
      viewport: { width: 390, height: 700 },
    },
  },
}

export const CurrentValuationFallback: Story = {
  name: '予想指標がない場合に現在の指標を表示する。',
  args: {
    valuation: { ...valuation, fwd_per: null, fwd_roe: null },
  },
}

export const ForecastValuation: Story = {
  name: '予想の PER と ROE を区別して表示する。',
  args: {
    valuation: { ...valuation, fwd_roe: 11 },
  },
}

export const WithoutPosition: Story = {
  name: '保有がない場合も企業情報を表示する。',
  args: { position: null },
}

export const MissingData: Story = {
  name: '次の決算とバリュエーションがない状態を表示する。',
  args: { earnings: null, valuation: null },
}

export const Loading: Story = {
  name: '企業情報を読み込んでいる状態を表示する。',
  args: {
    position: null,
    holdingsState: 'loading',
    earnings: null,
    earningsState: 'loading',
    valuation: null,
    valuationState: 'loading',
  },
}

export const Errors: Story = {
  name: '企業情報の取得に失敗した状態を表示する。',
  args: {
    position: null,
    holdingsState: 'error',
    earnings: null,
    earningsState: 'error',
    valuation: null,
    valuationState: 'error',
  },
}
