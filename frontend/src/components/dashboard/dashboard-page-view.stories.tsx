import type { Meta, StoryObj } from '@storybook/react-vite'
import { RouterProvider } from '@tanstack/react-router'
import type { ComponentProps, ComponentType } from 'react'

import { DashboardPageView } from '#components/dashboard/dashboard-page-view'
import { createStoryRouter } from '#storybook/story-router'

type DashboardPageViewProps = ComponentProps<typeof DashboardPageView>

const stocks: DashboardPageViewProps['stocks'] = [
  {
    id: '0000',
    name: 'デモ商事',
    market: null,
    sector_id: null,
    created_at: '2026-01-01T00:00:00Z',
    updated_at: '2026-01-01T00:00:00Z',
  },
  {
    id: '0001',
    name: 'サンプル工業',
    market: null,
    sector_id: null,
    created_at: '2026-01-01T00:00:00Z',
    updated_at: '2026-01-01T00:00:00Z',
  },
]

const args: DashboardPageViewProps = {
  stats: [
    { label: '総資産 (時価)', value: '¥1,255,300' },
    {
      label: '評価額 (株式・時価)',
      value: '¥629,000',
      sub: '簿価 ¥620,000',
    },
    {
      label: '含み損益',
      value: '+¥9,000',
      sub: '+1.5%',
      cls: 'text-up',
    },
    { label: '現金', value: '¥626,300', sub: '49.9% 比率' },
    { label: '実現損益 (累計)', value: '+¥12,300', cls: 'text-up' },
  ],
  isStatsError: false,
  positions: [
    {
      symbol: '0000',
      qty: 100,
      avg_cost: 3200,
      cost_basis: 320000,
      current_price: 3350,
      market_value: 335000,
      unrealized_pnl: 15000,
      realized_pnl: 0,
    },
    {
      symbol: '0001',
      qty: 200,
      avg_cost: 1500,
      cost_basis: 300000,
      current_price: 1470,
      market_value: 294000,
      unrealized_pnl: -6000,
      realized_pnl: -2500,
    },
  ],
  isPositionsError: false,
  stocks,
  reviewQueue: {
    counts: { notes: 1, annotations: 1, total: 2 },
    items: [
      {
        kind: 'note',
        id: 'fake-version-a',
        noteId: 'fake-note-a',
        versionNo: 2,
        title: '架空企業の決算前メモ',
        createdAt: '2026-10-10T03:00:00Z',
        noteKind: '架空の種別',
        createdByKind: 'llm',
        unreadVersionCount: 1,
      },
      {
        kind: 'annotation',
        id: 'fake-annotation-a',
        title: '出来高を確認する価格帯',
        createdAt: '2026-10-09T03:00:00Z',
        targetSymbol: 'DEMO-A',
        timestamp: '2026-10-09T03:00:00Z',
        price: null,
      },
    ],
  },
  isReviewPending: false,
  isReviewError: false,
  trades: [
    {
      id: 'fake-trade-a',
      strategy_id: 'fake-strategy-a',
      symbol: '0000',
      side: 'buy',
      qty: 100,
      price: 3200,
      fee: 0,
      date: '2026-10-08',
      source: 'manual',
      note: null,
      note_count: 1,
      created_at: '2026-10-08T02:00:00Z',
      updated_at: '2026-10-08T02:00:00Z',
    },
  ],
  isTradesError: false,
  events: [
    {
      kind: 'event',
      event_date: '2026-10-14',
      event_at: null,
      time_of_day: '引け後',
      country: 'JP',
      category: 'earnings',
      title: '四半期決算',
      source: 'fixture',
      external_id: 'fake-event-a',
      stock_id: '0000',
      fiscal_period: null,
    },
  ],
  isEventsError: false,
  notes: [
    {
      id: 'fake-note-a',
      version_id: 'fake-version-a',
      version_no: 1,
      is_current: true,
      title: '架空企業の決算前メモ',
      body_md: '表示用の架空ノートです。',
      frontmatter_json: {},
      resolved_price_references_json: {},
      graphs_json: [],
      tags: [],
      kind: null,
      status: 'approved',
      trigger: null,
      trigger_label: null,
      created_by_kind: 'llm',
      created_at: '2026-10-10T03:00:00Z',
      updated_at: '2026-10-10T03:00:00Z',
    },
    {
      id: 'fake-note-b',
      version_id: 'fake-version-b',
      version_no: 1,
      is_current: true,
      title: '売買後の振り返り',
      body_md: '表示用の架空ノートです。',
      frontmatter_json: {},
      resolved_price_references_json: {},
      graphs_json: [],
      tags: [],
      kind: null,
      status: 'approved',
      trigger: null,
      trigger_label: null,
      created_by_kind: 'human',
      created_at: '2026-10-09T03:00:00Z',
      updated_at: '2026-10-09T03:00:00Z',
    },
  ],
  isNotesError: false,
  tasks: [
    {
      task_id: 'fake-task-a',
      strategy_id: 'fake-strategy-a',
      source: 'frontend',
      prompt: '決算前の確認',
      phase: 'running',
      purpose: null,
      as_of: null,
      error_summary: null,
      created_at: '2026-10-10T03:00:00Z',
      updated_at: '2026-10-10T03:00:00Z',
    },
  ],
  isTasksError: false,
  strategies: [
    {
      id: 'fake-strategy-a',
      name: 'デモ戦略',
      description: null,
      sort_order: 0,
      created_at: '2026-01-01T00:00:00Z',
      updated_at: '2026-01-01T00:00:00Z',
    },
  ],
}

const paths = [
  '/portfolio',
  '/stocks/$stockId',
  '/trades',
  '/trades/$tradeId',
  '/calendar',
  '/notes',
  '/notes/$noteId',
  '/annotations/$annoId',
  '/runs',
  '/strategies/$id/runs/$taskId',
]

function withRouter(Story: ComponentType) {
  return (
    <RouterProvider
      router={createStoryRouter(
        () => (
          <div className="min-h-screen bg-background p-3 md:p-6">
            <Story />
          </div>
        ),
        { paths },
      )}
    />
  )
}

const meta = {
  title: 'Dashboard/DashboardPageView',
  component: DashboardPageView,
  decorators: [withRouter],
  parameters: { layout: 'fullscreen' },
  args,
} satisfies Meta<typeof DashboardPageView>

export default meta
type Story = StoryObj<typeof meta>

export const Populated: Story = {
  name: 'shows portfolio, upcoming activity, and review items together.',
}

export const Empty: Story = {
  name: 'shows the dashboard when no portfolio activity exists.',
  args: {
    ...args,
    stats: [
      { label: '総資産 (時価)', value: '¥0' },
      { label: '評価額 (株式・時価)', value: '¥0', sub: '簿価 ¥0' },
      {
        label: '含み損益',
        value: '¥0',
        sub: '—',
        cls: 'text-muted-foreground-strong',
      },
      { label: '現金', value: '¥0', sub: '— 比率' },
      { label: '実現損益 (累計)', value: '¥0' },
    ],
    positions: [],
    reviewQueue: {
      counts: { notes: 0, annotations: 0, total: 0 },
      items: [],
    },
    trades: [],
    events: [],
    notes: [],
    tasks: [],
  },
}

export const MissingMarketValue: Story = {
  name: 'shows unavailable portfolio totals and holdings without observed prices.',
  args: {
    ...args,
    stats: [
      { label: '総資産 (時価)', value: '—' },
      {
        label: '評価額 (株式・時価)',
        value: '—',
        sub: '簿価 ¥620,000',
      },
      {
        label: '含み損益',
        value: '—',
        sub: '—',
        cls: 'text-muted-foreground-strong',
      },
      { label: '現金', value: '¥626,300', sub: '68.1% 既知額比' },
      { label: '実現損益 (累計)', value: '+¥12,300', cls: 'text-up' },
    ],
    positions: [
      {
        symbol: '0000',
        qty: 100,
        avg_cost: 3200,
        cost_basis: 320000,
        current_price: null,
        market_value: null,
        unrealized_pnl: null,
        realized_pnl: 0,
      },
      {
        symbol: '0001',
        qty: 200,
        avg_cost: 1500,
        cost_basis: 300000,
        current_price: 1470,
        market_value: 294000,
        unrealized_pnl: -6000,
        realized_pnl: -2500,
      },
    ],
  },
}

export const Loading: Story = {
  name: 'shows loading placeholders while dashboard data is fetched.',
  args: {
    ...args,
    stats: null,
    positions: null,
    reviewQueue: null,
    isReviewPending: true,
    trades: null,
    events: null,
    notes: null,
    tasks: null,
  },
}

export const PortfolioError: Story = {
  name: 'shows an error when portfolio data cannot be loaded.',
  args: {
    ...args,
    stats: null,
    isStatsError: true,
    positions: null,
    isPositionsError: true,
  },
}

export const DataErrors: Story = {
  name: 'shows errors for sections whose data cannot be loaded.',
  args: {
    ...args,
    isStatsError: true,
    isPositionsError: true,
    isReviewError: true,
    isTradesError: true,
    isEventsError: true,
    isNotesError: true,
    isTasksError: true,
  },
}
