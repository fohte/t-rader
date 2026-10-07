import type { Meta, StoryObj } from '@storybook/react-vite'
import { RouterProvider } from '@tanstack/react-router'

import type { CalendarWeekRange } from '#components/calendar/calendar-date'
import { CalendarPageView } from '#components/calendar/calendar-page-view'
import type { components } from '#lib/api/schema.gen'
import { createStoryRouter } from '#storybook/story-router'

type CalendarEvent = components['schemas']['CalendarEventResponse']
type Strategy = components['schemas']['Strategy']

const STRATEGY_ID = '00000000-0000-0000-0000-000000000001'
const weekRange: CalendarWeekRange = {
  from: '2099-01-05',
  to: '2099-01-11',
}

const strategies: Strategy[] = [
  {
    id: STRATEGY_ID,
    name: '検証用戦略',
    description: null,
    sort_order: 0,
    created_at: '2099-01-01T00:00:00Z',
    updated_at: '2099-01-01T00:00:00Z',
  },
]

const events: CalendarEvent[] = [
  {
    kind: 'event',
    source: 'boj',
    external_id: 'sample-central-bank-event',
    category: 'central_bank',
    country: 'JP',
    title: '架空会合',
    stock_id: null,
    fiscal_period: null,
    event_date: '2099-01-06',
    event_at: '2099-01-06T00:00:00Z',
    time_of_day: null,
  },
  {
    kind: 'event',
    source: 'fred',
    external_id: '10:2099-01-06',
    category: 'indicator',
    country: 'US',
    title: '架空物価指標',
    stock_id: null,
    fiscal_period: null,
    event_date: '2099-01-06',
    event_at: '2099-01-06T13:30:00Z',
    time_of_day: null,
  },
  {
    kind: 'event',
    source: 'alpha_vantage',
    external_id: 'sample-pre-market-earnings',
    category: 'earnings',
    country: 'US',
    title: '架空電子',
    stock_id: 'FICT1',
    fiscal_period: '2099-01-01',
    event_date: '2099-01-06',
    event_at: null,
    time_of_day: 'pre_market',
  },
  {
    kind: 'event',
    source: 'jquants',
    external_id: 'sample-stock-earnings',
    category: 'earnings',
    country: 'JP',
    title: '架空工業',
    stock_id: '0000',
    fiscal_period: '2099-01-01',
    event_date: '2099-01-06',
    event_at: null,
    time_of_day: 'post_market',
  },
  {
    kind: 'other_earnings_summary',
    country: 'JP',
    event_date: '2099-01-06',
    count: 7,
  },
]

const otherEarningsEvents: CalendarEvent[] = [
  {
    kind: 'event',
    source: 'jquants',
    external_id: 'sample-other-earnings-a',
    category: 'earnings',
    country: 'JP',
    title: '架空機械',
    stock_id: '0001',
    fiscal_period: '2099-01-01',
    event_date: '2099-01-06',
    event_at: null,
    time_of_day: 'pre_market',
  },
  {
    kind: 'event',
    source: 'jquants',
    external_id: 'sample-other-earnings-b',
    category: 'earnings',
    country: 'JP',
    title: '架空素材',
    stock_id: '0002',
    fiscal_period: '2099-01-01',
    event_date: '2099-01-06',
    event_at: null,
    time_of_day: 'post_market',
  },
]

function createCalendarRouter(component: () => React.ReactNode) {
  return createStoryRouter(component, {
    paths: ['/calendar', '/charts/$instrumentId'],
    initialPath: '/calendar',
  })
}

const meta = {
  title: 'Calendar/CalendarPageView',
  component: CalendarPageView,
  parameters: { layout: 'fullscreen' },
  args: {
    calendar: { from: weekRange.from, to: weekRange.to, events },
    weekRange,
    strategies,
    selectedStrategyId: STRATEGY_ID,
    countryFilter: 'all',
    isPending: false,
    errorMessage: undefined,
    strategyErrorMessage: undefined,
    expandedOtherEarnings: undefined,
    otherEarningsEvents: undefined,
    isOtherEarningsPending: false,
    otherEarningsErrorMessage: undefined,
    onPreviousWeek: () => {},
    onNextWeek: () => {},
    onStrategyChange: () => {},
    onCountryChange: () => {},
    onOtherEarningsToggle: () => {},
  },
  render: (args) => (
    <RouterProvider
      router={createCalendarRouter(() => (
        <CalendarPageView {...args} />
      ))}
    />
  ),
} satisfies Meta<typeof CalendarPageView>

export default meta
type Story = StoryObj<typeof meta>

export const WithEvents: Story = {
  name: '選択した週のイベントと対象銘柄を表示します。',
}

export const Loading: Story = {
  name: 'イベントを読み込んでいます。',
  args: { isPending: true, calendar: undefined },
}

export const Empty: Story = {
  name: '選択した週に予定がありません。',
  args: { calendar: { from: weekRange.from, to: weekRange.to, events: [] } },
}

export const Error: Story = {
  name: 'イベントを取得できませんでした。',
  args: { errorMessage: 'イベントの取得に失敗しました' },
}

export const StrategyError: Story = {
  name: '戦略一覧の取得に失敗しました。',
  args: { strategyErrorMessage: '戦略一覧の取得に失敗しました' },
}

export const OtherEarningsExpanded: Story = {
  name: '他社の決算一覧を展開しています。',
  args: {
    calendar: {
      from: weekRange.from,
      to: weekRange.to,
      events: events.map((event) =>
        event.kind === 'other_earnings_summary'
          ? { ...event, count: 2 }
          : event,
      ),
    },
    expandedOtherEarnings: { country: 'JP', eventDate: '2099-01-06' },
    otherEarningsEvents,
  },
}

export const OtherEarningsLoading: Story = {
  name: '他社の決算一覧を読み込んでいます。',
  args: {
    expandedOtherEarnings: { country: 'JP', eventDate: '2099-01-06' },
    isOtherEarningsPending: true,
  },
}

export const OtherEarningsEmpty: Story = {
  name: '他社の決算が見つかりませんでした。',
  args: {
    calendar: {
      from: weekRange.from,
      to: weekRange.to,
      events: events.map((event) =>
        event.kind === 'other_earnings_summary'
          ? { ...event, count: 1 }
          : event,
      ),
    },
    expandedOtherEarnings: { country: 'JP', eventDate: '2099-01-06' },
    otherEarningsEvents: [],
  },
}

export const OtherEarningsError: Story = {
  name: '他社の決算一覧を取得できませんでした。',
  args: {
    expandedOtherEarnings: { country: 'JP', eventDate: '2099-01-06' },
    otherEarningsErrorMessage: '決算一覧の取得に失敗しました',
  },
}
