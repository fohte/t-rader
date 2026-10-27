import { describe, expect, it } from 'vitest'

import { buildCalendarDays } from '#components/calendar/calendar-model'
import type { components } from '#lib/api/schema.gen'

type CalendarEvent = components['schemas']['CalendarEventResponse']
type Event = Extract<CalendarEvent, { kind: 'event' }>
type EventReference = components['schemas']['CalendarEventReferenceResponse']

const EVENT_DATE = '2099-01-06'
const TODAY = '2099-01-07'

function makeIndicator(
  country: 'JP' | 'US',
  references: EventReference[],
): Event {
  return {
    kind: 'event',
    source: country === 'JP' ? 'e_stat' : 'fred',
    external_id: 'sample-indicator-event',
    category: 'indicator',
    country,
    title: '架空経済指標',
    stock_id: null,
    fiscal_period: null,
    fiscal_period_type: null,
    has_matching_earnings_summary: false,
    event_date: EVENT_DATE,
    event_at: null,
    time_of_day: null,
    references,
  }
}

function makeReference(
  kind: EventReference['kind'],
  url: string | null,
): EventReference {
  return {
    kind,
    source: 'sample_source',
    external_id: 'sample-reference',
    title: '架空の発表資料',
    url,
    target_period_start: null,
    target_period_end: kind === 'individual_document' ? '2098-12-31' : null,
    target_period_type: kind === 'individual_document' ? 'month' : null,
    publication_evidence:
      kind === 'individual_document' ? '架空の公表記録' : null,
    publication_evidence_url: null,
    checked_at: '2099-01-06T00:00:00Z',
  }
}

const individualDocument = makeReference(
  'individual_document',
  'https://example.invalid/sample-document',
)
const officialListing = makeReference(
  'official_listing',
  'https://example.invalid/sample-listing',
)
const documentWithoutUrl = makeReference('individual_document', null)
const halfYearDocument = {
  ...individualDocument,
  target_period_start: '2098-07-01',
  target_period_type: 'half_year',
}

const referenceViews = {
  individual: [
    {
      key: 'individual_document:sample_source:sample-reference',
      title: '架空の発表資料',
      source: 'sample_source',
      checkedAt: '2099-01-06T00:00:00Z',
      targetPeriodEnd: '2098-12-31',
      targetPeriodType: 'month',
      href: 'https://example.invalid/sample-document',
      publicationEvidence: '架空の公表記録',
    },
  ],
  halfYear: [
    {
      key: 'individual_document:sample_source:sample-reference',
      title: '架空の発表資料',
      source: 'sample_source',
      checkedAt: '2099-01-06T00:00:00Z',
      targetPeriodStart: '2098-07-01',
      targetPeriodEnd: '2098-12-31',
      targetPeriodType: '半期',
      href: 'https://example.invalid/sample-document',
      publicationEvidence: '架空の公表記録',
    },
  ],
  listing: [
    {
      key: 'official_listing:sample_source:sample-reference',
      title: '架空の発表資料',
      source: 'sample_source',
      checkedAt: '2099-01-06T00:00:00Z',
      href: 'https://example.invalid/sample-listing',
    },
  ],
}

describe('buildCalendarDays indicator references', () => {
  it.each([
    {
      name: 'individual document',
      country: 'JP' as const,
      references: [individualDocument],
      label: '個別資料あり',
      expectedReferences: referenceViews.individual,
    },
    {
      name: 'half-year document period',
      country: 'JP' as const,
      references: [halfYearDocument],
      label: '個別資料あり',
      expectedReferences: referenceViews.halfYear,
    },
    {
      name: 'official listing only',
      country: 'US' as const,
      references: [officialListing],
      label: '公式一覧 / 個別資料未確認',
      expectedReferences: referenceViews.listing,
    },
    {
      name: 'unconfirmed destination',
      country: 'JP' as const,
      references: [],
      label: '遷移先未確認 / 非リンク',
      expectedReferences: [],
    },
    {
      name: 'document without a destination',
      country: 'US' as const,
      references: [documentWithoutUrl],
      label: '遷移先未確認 / 非リンク',
      expectedReferences: [],
    },
  ])(
    'shows $name status for $country indicators',
    ({ country, references, label, expectedReferences }) => {
      const event = makeIndicator(country, references)

      expect(buildCalendarDays([event], 'all', undefined, TODAY)).toEqual([
        {
          date: EVENT_DATE,
          label: '1/6 (火)',
          rows: [
            {
              key: 'sample-indicator-event',
              time: '',
              country,
              category: '指標',
              title: '架空経済指標',
              stockId: undefined,
              indicatorStatus: { label, references: expectedReferences },
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
})
