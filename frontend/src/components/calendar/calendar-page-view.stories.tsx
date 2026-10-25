import type { Meta, StoryObj } from '@storybook/react-vite'
import { RouterProvider } from '@tanstack/react-router'

import type { CalendarWeekRange } from '#components/calendar/calendar-date'
import { CalendarPageView } from '#components/calendar/calendar-page-view'
import type { components } from '#lib/api/schema.gen'
import { createStoryRouter } from '#storybook/story-router'

type CalendarEvent = components['schemas']['CalendarEventResponse']
type Strategy = components['schemas']['Strategy']
const EVENT_REFERENCE_DEFAULTS = {
  fiscal_period_type: null,
  has_matching_earnings_summary: false,
  references: [],
} satisfies Pick<
  Extract<CalendarEvent, { kind: 'event' }>,
  'fiscal_period_type' | 'has_matching_earnings_summary' | 'references'
>

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
    ...EVENT_REFERENCE_DEFAULTS,
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
    ...EVENT_REFERENCE_DEFAULTS,
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
    ...EVENT_REFERENCE_DEFAULTS,
    kind: 'event',
    source: 'alpha_vantage',
    external_id: 'sample-pre-market-earnings',
    category: 'earnings',
    country: 'US',
    title: '架空電子',
    stock_id: 'US:FICT1',
    fiscal_period: '2099-01-01',
    event_date: '2099-01-06',
    event_at: null,
    time_of_day: 'pre_market',
  },
  {
    ...EVENT_REFERENCE_DEFAULTS,
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
    ...EVENT_REFERENCE_DEFAULTS,
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
    ...EVENT_REFERENCE_DEFAULTS,
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
    paths: ['/calendar', '/stocks/$stockId'],
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

export const StrategyFilterOpen: Story = {
  name: '戦略の選択肢を表示しています。',
  args: { openFilter: 'strategy' },
}

export const CountryFilterOpen: Story = {
  name: '国の選択肢を表示しています。',
  args: { openFilter: 'country' },
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

export const MobileOverseasEarningsExpanded: Story = {
  name: '390 px 幅で海外決算の展開行と銘柄リンクを表示します。',
  args: {
    calendar: {
      from: weekRange.from,
      to: weekRange.to,
      events: [
        {
          ...EVENT_REFERENCE_DEFAULTS,
          kind: 'event',
          source: 'alpha_vantage',
          external_id: 'sample-mobile-overseas-earnings',
          category: 'earnings',
          country: 'US',
          title: '架空電子',
          stock_id: 'US:FICT1',
          fiscal_period: '2099-01-01',
          event_date: '2099-01-06',
          event_at: null,
          time_of_day: 'pre_market',
        },
        {
          kind: 'other_earnings_summary',
          country: 'US',
          event_date: '2099-01-06',
          count: 2,
        },
      ],
    },
    expandedOtherEarnings: { country: 'US', eventDate: '2099-01-06' },
    otherEarningsEvents: [
      {
        ...EVENT_REFERENCE_DEFAULTS,
        kind: 'event',
        source: 'alpha_vantage',
        external_id: 'sample-mobile-other-earnings-a',
        category: 'earnings',
        country: 'US',
        title: '架空地域情報通信設備総合開発研究製造会社',
        stock_id: 'US:FICT2',
        fiscal_period: '2099-01-01',
        event_date: '2099-01-06',
        event_at: null,
        time_of_day: 'pre_market',
      },
      {
        ...EVENT_REFERENCE_DEFAULTS,
        kind: 'event',
        source: 'alpha_vantage',
        external_id: 'sample-mobile-other-earnings-b',
        category: 'earnings',
        country: 'US',
        title: '架空広域通信設備開発会社',
        stock_id: 'US:FICT3',
        fiscal_period: '2099-01-01',
        event_date: '2099-01-06',
        event_at: null,
        time_of_day: 'post_market',
      },
    ],
  },
  globals: { viewport: { value: 'mobileCalendar' } },
  parameters: {
    viewport: {
      options: {
        mobileCalendar: {
          name: 'Mobile calendar',
          styles: { width: '390px', height: '700px' },
          type: 'mobile',
        },
      },
    },
    screenshot: { viewport: { width: 390, height: 700 } },
  },
}

export const MobileDomesticResultsExpanded: Story = {
  name: '390 px 幅で対象期実績と参照資料の導線を表示します。',
  args: {
    calendar: {
      from: weekRange.from,
      to: weekRange.to,
      events: [
        {
          ...EVENT_REFERENCE_DEFAULTS,
          kind: 'event',
          source: 'jquants',
          external_id: '0000:2Q:2099-06-30',
          category: 'earnings',
          country: 'JP',
          title: '架空工業',
          stock_id: '0000',
          fiscal_period: '2099-06-30',
          fiscal_period_type: '2Q',
          event_date: '2099-01-06',
          event_at: null,
          time_of_day: 'post_market',
          references: [
            {
              kind: 'earnings_summary',
              source: 'sample_source',
              external_id: 'sample-summary',
              title: '架空の財務要約',
              url: 'https://example.invalid/sample-summary',
              target_period_start: null,
              target_period_end: '2099-06-30',
              target_period_type: '2Q',
              publication_evidence: null,
              publication_evidence_url: null,
              checked_at: '2030-01-01T00:00:00Z',
            },
          ],
          has_matching_earnings_summary: true,
        },
        {
          kind: 'other_earnings_summary',
          country: 'JP',
          event_date: '2099-01-06',
          count: 1,
        },
      ],
    },
    expandedOtherEarnings: { country: 'JP', eventDate: '2099-01-06' },
    otherEarningsEvents: [
      {
        ...EVENT_REFERENCE_DEFAULTS,
        kind: 'event',
        source: 'jquants',
        external_id: '0001:2Q:2099-06-30',
        category: 'earnings',
        country: 'JP',
        title: '架空素材',
        stock_id: '0001',
        fiscal_period: '2099-06-30',
        fiscal_period_type: '2Q',
        event_date: '2099-01-06',
        event_at: null,
        time_of_day: 'pre_market',
        has_matching_earnings_summary: true,
      },
    ],
  },
  globals: { viewport: { value: 'mobileCalendar' } },
  parameters: {
    viewport: {
      options: {
        mobileCalendar: {
          name: 'Mobile calendar',
          styles: { width: '390px', height: '700px' },
          type: 'mobile',
        },
      },
    },
    screenshot: { viewport: { width: 390, height: 700 } },
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
