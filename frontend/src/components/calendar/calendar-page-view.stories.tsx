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
    onPreviousWeek: () => {},
    onNextWeek: () => {},
    onStrategyChange: () => {},
    onCountryChange: () => {},
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
