import { describe, expect, it } from 'vitest'

import { buildCalendarDays } from '#components/calendar/calendar-model'
import type { components } from '#lib/api/schema.gen'

type CalendarEvent = components['schemas']['CalendarEventResponse']

const events: CalendarEvent[] = [
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
]

describe('buildCalendarDays', () => {
  it('groups events by date and formats strategy earnings for the calendar', () => {
    expect(
      buildCalendarDays(events, 'all', '00000000-0000-0000-0000-000000000001'),
    ).toEqual([
      {
        date: '2099-01-06',
        label: '1/6 (火)',
        rows: [
          {
            key: 'sample-pre-market-earnings',
            time: '寄り前',
            country: 'US',
            category: '決算',
            title: '架空電子',
            stockId: undefined,
            emphasized: false,
            target: true,
          },
          {
            key: 'sample-central-bank-event',
            time: '09:00',
            country: 'JP',
            category: '中銀',
            title: '架空会合',
            stockId: undefined,
            emphasized: true,
            target: false,
          },
          {
            key: '10:2099-01-06',
            time: '22:30',
            country: 'US',
            category: '指標',
            title: '架空物価指標',
            stockId: undefined,
            emphasized: true,
            target: false,
          },
          {
            key: 'sample-stock-earnings',
            time: '引け後',
            country: 'JP',
            category: '決算',
            title: '架空工業 (0000)',
            stockId: '0000',
            emphasized: false,
            target: true,
          },
          {
            key: 'JP:2099-01-06:other-earnings',
            time: '',
            country: 'JP',
            category: '決算',
            title: '他 7 社',
            emphasized: false,
            target: false,
          },
        ],
      },
    ])
  })

  it('filters out rows from countries that are not selected', () => {
    expect(buildCalendarDays(events, 'JP', undefined)).toEqual([
      {
        date: '2099-01-06',
        label: '1/6 (火)',
        rows: [
          {
            key: 'sample-central-bank-event',
            time: '09:00',
            country: 'JP',
            category: '中銀',
            title: '架空会合',
            stockId: undefined,
            emphasized: true,
            target: false,
          },
          {
            key: 'sample-stock-earnings',
            time: '引け後',
            country: 'JP',
            category: '決算',
            title: '架空工業 (0000)',
            stockId: '0000',
            emphasized: false,
            target: false,
          },
          {
            key: 'JP:2099-01-06:other-earnings',
            time: '',
            country: 'JP',
            category: '決算',
            title: '他 7 社',
            emphasized: false,
            target: false,
          },
        ],
      },
    ])
  })
})
