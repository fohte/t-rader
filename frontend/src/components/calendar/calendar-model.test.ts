import { describe, expect, it } from 'vitest'

import { buildCalendarDays } from '#components/calendar/calendar-model'
import type { components } from '#lib/api/schema.gen'

type CalendarEvent = components['schemas']['CalendarEventResponse']
type EarningsEvent = Extract<CalendarEvent, { kind: 'event' }>
type EventReference = components['schemas']['CalendarEventReferenceResponse']

const EMPTY_EARNINGS_DATA: Pick<
  EarningsEvent,
  'fiscal_period_type' | 'has_matching_earnings_summary' | 'references'
> = {
  fiscal_period_type: null,
  has_matching_earnings_summary: false,
  references: [],
}

const events: CalendarEvent[] = [
  {
    ...EMPTY_EARNINGS_DATA,
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
    ...EMPTY_EARNINGS_DATA,
    kind: 'event',
    source: 'alpha_vantage',
    external_id: 'sample-post-market-earnings',
    category: 'earnings',
    country: 'US',
    title: '架空測定',
    stock_id: 'US:FICT2',
    fiscal_period: '2099-01-01',
    event_date: '2099-01-06',
    event_at: null,
    time_of_day: 'post_market',
  },
  {
    ...EMPTY_EARNINGS_DATA,
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
    ...EMPTY_EARNINGS_DATA,
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
    ...EMPTY_EARNINGS_DATA,
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
  it.each([
    {
      name: 'summary for the target period',
      kind: 'earnings_summary',
      targetPeriodEnd: '2099-06-30',
      targetPeriodType: '2Q',
      expectedStatus: '実績あり',
    },
    {
      name: 'summary with unknown event period type',
      kind: 'earnings_summary',
      targetPeriodEnd: '2099-06-30',
      targetPeriodType: '2Q',
      eventPeriodType: null,
      expectedStatus: '実績・資料未取得 / 公表未確認',
    },
    {
      name: 'summary for another period',
      kind: 'earnings_summary',
      targetPeriodEnd: '2099-06-29',
      targetPeriodType: '2Q',
      expectedStatus: '実績・資料未取得 / 公表未確認',
    },
    {
      name: 'summary for another quarter',
      kind: 'earnings_summary',
      targetPeriodEnd: '2099-06-30',
      targetPeriodType: '1Q',
      expectedStatus: '実績・資料未取得 / 公表未確認',
    },
    {
      name: 'individual document without a summary',
      kind: 'individual_document',
      targetPeriodEnd: '2099-06-30',
      targetPeriodType: '2Q',
      expectedStatus: '資料あり / 実績未取得',
    },
    {
      name: 'official listing without a document',
      kind: 'official_listing',
      targetPeriodEnd: '2099-06-30',
      targetPeriodType: '2Q',
      expectedStatus: '個別資料未確認',
    },
  ] as const)(
    'labels $name without inferring status from the event date',
    ({
      kind,
      targetPeriodEnd,
      targetPeriodType,
      eventPeriodType = '2Q',
      expectedStatus,
    }) => {
      const reference: EventReference = {
        kind,
        source: 'sample_source',
        external_id: 'sample-reference',
        title: '架空決算資料',
        url: 'https://example.invalid/sample-document',
        target_period_start: null,
        target_period_end: targetPeriodEnd,
        target_period_type: targetPeriodType,
        publication_evidence:
          kind === 'individual_document' ? 'sample-release-record' : null,
        publication_evidence_url: null,
        checked_at: '2030-01-01T00:00:00Z',
      }
      const event: EarningsEvent = {
        ...EMPTY_EARNINGS_DATA,
        kind: 'event',
        source: 'jquants',
        external_id: '0000:2Q:2099-06-30',
        category: 'earnings',
        country: 'JP',
        title: '架空工業',
        stock_id: '0000',
        fiscal_period: '2099-06-30',
        fiscal_period_type: eventPeriodType,
        event_date: '2099-01-06',
        event_at: null,
        time_of_day: 'post_market',
        references: [reference],
      }
      const expectedReference = {
        key: `${kind}:sample_source:sample-reference`,
        title: '架空決算資料',
        source: 'sample_source',
        checkedAt: '2030-01-01T00:00:00Z',
        targetPeriodEnd,
        targetPeriodType,
        href: 'https://example.invalid/sample-document',
        ...(kind === 'individual_document'
          ? { publicationEvidence: 'sample-release-record' }
          : {}),
      }

      expect(buildCalendarDays([event], 'all', undefined)).toEqual([
        {
          date: '2099-01-06',
          label: '1/6 (火)',
          rows: [
            {
              key: '0000:2Q:2099-06-30',
              time: '引け後',
              country: 'JP',
              category: '決算',
              title: '架空工業 (0000)',
              stockId: '0000',
              earningsStatus: {
                label: expectedStatus,
                references: [expectedReference],
              },
              emphasized: false,
              target: false,
              muted: false,
              otherEarnings: undefined,
            },
          ],
        },
      ])
    },
  )

  it('groups events by date and formats strategy earnings for the calendar', () => {
    expect(
      buildCalendarDays(events, 'all', '00000000-0000-0000-0000-000000000001'),
    ).toEqual([
      {
        date: '2099-01-06',
        label: '1/6 (火)',
        rows: [
          {
            key: 'sample-post-market-earnings',
            time: '引け後',
            country: 'US',
            category: '決算',
            title: '架空測定',
            stockId: 'US:FICT2',
            earningsStatus: {
              label: '実績・資料未取得 / 公表未確認',
              references: [],
            },
            emphasized: false,
            target: true,
            muted: false,
            otherEarnings: undefined,
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
            muted: false,
            otherEarnings: undefined,
          },
          {
            key: 'sample-stock-earnings',
            time: '引け後',
            country: 'JP',
            category: '決算',
            title: '架空工業 (0000)',
            stockId: '0000',
            earningsStatus: {
              label: '実績・資料未取得 / 公表未確認',
              references: [],
            },
            emphasized: false,
            target: true,
            muted: false,
            otherEarnings: undefined,
          },
          {
            key: 'sample-pre-market-earnings',
            time: '寄り前',
            country: 'US',
            category: '決算',
            title: '架空電子',
            stockId: 'US:FICT1',
            earningsStatus: {
              label: '実績・資料未取得 / 公表未確認',
              references: [],
            },
            emphasized: false,
            target: true,
            muted: false,
            otherEarnings: undefined,
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
            muted: false,
            otherEarnings: undefined,
          },
          {
            key: 'JP:2099-01-06:other-earnings',
            time: '',
            country: 'JP',
            category: '決算',
            title: '他 7 社',
            emphasized: false,
            target: false,
            muted: true,
            otherEarnings: { country: 'JP', eventDate: '2099-01-06' },
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
            muted: false,
            otherEarnings: undefined,
          },
          {
            key: 'sample-stock-earnings',
            time: '引け後',
            country: 'JP',
            category: '決算',
            title: '架空工業 (0000)',
            stockId: '0000',
            earningsStatus: {
              label: '実績・資料未取得 / 公表未確認',
              references: [],
            },
            emphasized: false,
            target: false,
            muted: false,
            otherEarnings: undefined,
          },
          {
            key: 'JP:2099-01-06:other-earnings',
            time: '',
            country: 'JP',
            category: '決算',
            title: '他 7 社',
            stockId: undefined,
            emphasized: false,
            target: false,
            muted: true,
            otherEarnings: { country: 'JP', eventDate: '2099-01-06' },
          },
        ],
      },
    ])
  })
})
