import { describe, expect, it } from 'vitest'

import { buildCalendarDays } from '#components/calendar/calendar-model'
import type { components } from '#lib/api/schema.gen'

type CalendarEvent = components['schemas']['CalendarEventResponse']
type Event = Extract<CalendarEvent, { kind: 'event' }>
type EventReference = components['schemas']['CalendarEventReferenceResponse']

const TODAY = '2099-01-07'

function makeCentralBankEvent(overrides: Partial<Event>): Event {
  return {
    kind: 'event',
    source: 'boj',
    external_id: 'meeting-2099-01-06',
    category: 'central_bank',
    country: 'JP',
    title: '架空金融政策決定会合 (2日目)',
    stock_id: null,
    fiscal_period: null,
    fiscal_period_type: null,
    has_matching_earnings_summary: false,
    event_date: '2099-01-06',
    event_at: null,
    time_of_day: null,
    references: [],
    ...overrides,
  }
}

function makeReference(
  kind: EventReference['kind'],
  url: string | null,
  periodType: string | null = null,
): EventReference {
  return {
    kind,
    source: 'sample_authority',
    external_id: `sample-${kind}`,
    title:
      kind === 'individual_document' ? '架空の個別資料' : '架空の公式資料一覧',
    url,
    target_period_start:
      kind === 'individual_document' && periodType != null
        ? '2098-10-01'
        : null,
    target_period_end:
      kind === 'individual_document' && periodType != null
        ? '2098-12-31'
        : null,
    target_period_type: periodType,
    publication_evidence:
      kind === 'individual_document' ? '架空の公表記録' : null,
    publication_evidence_url: null,
    checked_at: '2099-01-06T00:00:00Z',
  }
}

const individualDocument = makeReference(
  'individual_document',
  'https://example.invalid/sample-decision',
  'quarter',
)
const officialSchedule = makeReference(
  'official_listing',
  'https://example.invalid/sample-schedule',
  'official_schedule',
)
const officialArchive = makeReference(
  'official_listing',
  'https://example.invalid/sample-archive',
  'official_archive',
)
const individualDocumentView = {
  key: 'individual_document:sample_authority:sample-individual_document',
  title: '架空の個別資料',
  source: 'sample_authority',
  checkedAt: '2099-01-06T00:00:00Z',
  targetPeriodStart: '2098-10-01',
  targetPeriodEnd: '2098-12-31',
  targetPeriodType: '四半期',
  href: 'https://example.invalid/sample-decision',
  publicationEvidence: '架空の公表記録',
}

describe('buildCalendarDays central bank references', () => {
  it.each([
    {
      name: 'keeps the first BOJ meeting day separate from decision material',
      event: makeCentralBankEvent({
        title: '架空金融政策決定会合 (1日目)',
        references: [individualDocument, officialArchive],
      }),
      phase: 'first_day',
      status: '公式一覧のみ',
      dateLabel: '1/6 (火)',
      time: '',
      references: [
        {
          key: 'official_listing:sample_authority:sample-official_listing',
          title: '架空の公式資料一覧',
          source: 'sample_authority',
          checkedAt: '2099-01-06T00:00:00Z',
          href: 'https://example.invalid/sample-archive',
          referenceTypeLabel: '公式一覧',
        },
      ],
    },
    {
      name: 'marks the BOJ meeting decision day with its individual material',
      event: makeCentralBankEvent({ references: [individualDocument] }),
      phase: 'decision_day',
      status: '個別資料あり',
      dateLabel: '1/6 (火)',
      time: '',
      references: [individualDocumentView],
    },
    {
      name: 'does not link BOJ meeting material when the title has no day marker',
      event: makeCentralBankEvent({
        title: '架空金融政策決定会合',
        references: [individualDocument, officialArchive],
      }),
      phase: undefined,
      status: '公式一覧のみ',
      dateLabel: '1/6 (火)',
      time: '',
      references: [
        {
          key: 'official_listing:sample_authority:sample-official_listing',
          title: '架空の公式資料一覧',
          source: 'sample_authority',
          checkedAt: '2099-01-06T00:00:00Z',
          href: 'https://example.invalid/sample-archive',
          referenceTypeLabel: '公式一覧',
        },
      ],
    },
    {
      name: 'does not link BOJ meeting material for an invalid day marker',
      event: makeCentralBankEvent({
        title: '架空金融政策決定会合 (0日目)',
        references: [individualDocument],
      }),
      phase: undefined,
      status: '遷移先未確認 / 非リンク',
      dateLabel: '1/6 (火)',
      time: '',
      references: [],
    },
    {
      name: 'shows a future Fed decision only with an official schedule reference',
      event: makeCentralBankEvent({
        source: 'fed',
        external_id: '2099-01-08',
        country: 'US',
        title: '架空金融政策会合の声明',
        event_date: '2099-01-08',
        references: [officialSchedule],
      }),
      phase: 'decision_day',
      status: '発表予定',
      dateLabel: '1/8 (木)',
      time: '',
      references: [
        {
          key: 'official_listing:sample_authority:sample-official_listing',
          title: '架空の公式資料一覧',
          source: 'sample_authority',
          checkedAt: '2099-01-06T00:00:00Z',
          href: 'https://example.invalid/sample-schedule',
          referenceTypeLabel: '公式日程',
        },
      ],
    },
    {
      name: 'does not infer an official schedule from a future event date alone',
      event: makeCentralBankEvent({
        source: 'fed',
        external_id: '2099-01-08',
        country: 'US',
        title: '架空金融政策会合の声明',
        event_date: '2099-01-08',
        references: [],
      }),
      phase: 'decision_day',
      status: '遷移先未確認 / 非リンク',
      dateLabel: '1/8 (木)',
      time: '',
      references: [],
    },
    {
      name: 'keeps ECB first-day events from linking decision material',
      event: makeCentralBankEvent({
        source: 'ecb',
        external_id: '2099-01-06',
        country: 'EU',
        title: '架空中央銀行理事会会合',
        event_at: null,
        references: [individualDocument, officialArchive],
      }),
      phase: 'first_day',
      status: '公式一覧のみ',
      dateLabel: '1/6 (火)',
      time: '',
      references: [
        {
          key: 'official_listing:sample_authority:sample-official_listing',
          title: '架空の公式資料一覧',
          source: 'sample_authority',
          checkedAt: '2099-01-06T00:00:00Z',
          href: 'https://example.invalid/sample-archive',
          referenceTypeLabel: '公式一覧',
        },
      ],
    },
    {
      name: 'marks an ECB decision date with an official listing only',
      event: makeCentralBankEvent({
        source: 'ecb',
        external_id: '2099-01-06',
        country: 'EU',
        title: '架空中央銀行理事会会合',
        event_at: '2099-01-06T13:15:00Z',
        references: [officialArchive],
      }),
      phase: 'decision_day',
      status: '公式一覧のみ',
      dateLabel: '1/6 (火)',
      time: '22:15',
      references: [
        {
          key: 'official_listing:sample_authority:sample-official_listing',
          title: '架空の公式資料一覧',
          source: 'sample_authority',
          checkedAt: '2099-01-06T00:00:00Z',
          href: 'https://example.invalid/sample-archive',
          referenceTypeLabel: '公式一覧',
        },
      ],
    },
    {
      name: 'keeps an unconfirmed decision destination non-linkable',
      event: makeCentralBankEvent({ references: [] }),
      phase: 'decision_day',
      status: '遷移先未確認 / 非リンク',
      dateLabel: '1/6 (火)',
      time: '',
      references: [],
    },
    {
      name: 'shows a future Tankan event as an official schedule',
      event: makeCentralBankEvent({
        external_id: 'tankan-2099-01-08',
        title: '架空短観',
        event_date: '2099-01-08',
        references: [officialSchedule],
      }),
      phase: undefined,
      status: '発表予定',
      dateLabel: '1/8 (木)',
      time: '',
      references: [
        {
          key: 'official_listing:sample_authority:sample-official_listing',
          title: '架空の公式資料一覧',
          source: 'sample_authority',
          checkedAt: '2099-01-06T00:00:00Z',
          href: 'https://example.invalid/sample-schedule',
          referenceTypeLabel: '公式日程',
        },
      ],
    },
    {
      name: 'omits unknown period types from a Tankan document',
      event: makeCentralBankEvent({
        external_id: 'tankan-2099-01-06',
        title: '架空短観',
        references: [
          makeReference(
            'individual_document',
            'https://example.invalid/sample-tankan',
            'unknown_period',
          ),
        ],
      }),
      phase: undefined,
      status: '個別資料あり',
      dateLabel: '1/6 (火)',
      time: '',
      references: [
        {
          key: 'individual_document:sample_authority:sample-individual_document',
          title: '架空の個別資料',
          source: 'sample_authority',
          checkedAt: '2099-01-06T00:00:00Z',
          targetPeriodStart: '2098-10-01',
          targetPeriodEnd: '2098-12-31',
          href: 'https://example.invalid/sample-tankan',
          publicationEvidence: '架空の公表記録',
        },
      ],
    },
  ])('$name', ({ event, phase, status, dateLabel, time, references }) => {
    expect(buildCalendarDays([event], 'all', undefined, TODAY)).toEqual([
      {
        date: event.event_date,
        label: dateLabel,
        rows: [
          {
            key: event.external_id,
            time,
            country: event.country,
            category: '中銀',
            title: event.title,
            stockId: undefined,
            centralBankStatus: {
              ...(phase == null ? {} : { phase }),
              label: status,
              references,
            },
            emphasized: true,
            target: false,
            muted: false,
            otherEarnings: undefined,
          },
        ],
      },
    ])
  })
})
