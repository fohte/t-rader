import {
  type CalendarCountryFilter,
  formatCalendarDayLabel,
} from '#components/calendar/calendar-date'
import type { components } from '#lib/api/schema.gen'
import type { FinancialPeriodType } from '#lib/stock-page-date'

type CalendarEvent = components['schemas']['CalendarEventResponse']
type Event = Extract<CalendarEvent, { kind: 'event' }>
type OtherEarningsSummary = Extract<
  CalendarEvent,
  { kind: 'other_earnings_summary' }
>
type EventReference = components['schemas']['CalendarEventReferenceResponse']

export type CalendarEarningsReferenceView = {
  key: string
  title: string
  source: string
  checkedAt: string
  targetPeriodEnd?: string
  targetPeriodType?: string
  href?: string
  publicationEvidence?: string
  publicationEvidenceHref?: string
}

export type CalendarEarningsStatus = {
  label: string
  internalTarget?: {
    stockId: string
    periodEnd: string
    periodType: FinancialPeriodType
  }
  references: CalendarEarningsReferenceView[]
}

export type OtherEarningsSelection = {
  country: string
  eventDate: string
}

export function isSameOtherEarningsSelection(
  current: OtherEarningsSelection | undefined,
  selection: OtherEarningsSelection,
): boolean {
  return (
    current?.country === selection.country &&
    current.eventDate === selection.eventDate
  )
}

type CalendarRowView = {
  key: string
  time: string
  country: string
  category: string
  title: string
  stockId?: string
  earningsStatus?: CalendarEarningsStatus
  emphasized: boolean
  target: boolean
  muted: boolean
  otherEarnings: OtherEarningsSelection | undefined
}

type CalendarDayView = {
  date: string
  label: string
  rows: CalendarRowView[]
}

const CATEGORY_LABELS: Readonly<Record<string, string>> = {
  central_bank: '中銀',
  earnings: '決算',
  indicator: '指標',
}

export function formatCalendarCategory(category: string): string {
  return CATEGORY_LABELS[category] ?? category
}

const EMPHASIZED_FRED_RELEASE_IDS: ReadonlySet<string> = new Set(['10', '50'])
const MARKET_TIME_SORT_ORDER: Readonly<
  Record<string, Readonly<Record<'pre_market' | 'post_market', number>>>
> = {
  JP: { pre_market: 8 * 60, post_market: 15 * 60 + 30 },
  US: { pre_market: 22 * 60, post_market: 5 * 60 },
}

function formatJstTime(eventAt: string): { label: string; minutes: number } {
  const parts = new Intl.DateTimeFormat('en-GB', {
    timeZone: 'Asia/Tokyo',
    hour: '2-digit',
    minute: '2-digit',
    hourCycle: 'h23',
  }).formatToParts(new Date(eventAt))
  const hour = Number(parts.find((part) => part.type === 'hour')?.value ?? 0)
  const minute = Number(
    parts.find((part) => part.type === 'minute')?.value ?? 0,
  )
  return {
    label: `${hour.toString().padStart(2, '0')}:${minute.toString().padStart(2, '0')}`,
    minutes: hour * 60 + minute,
  }
}

function getEventTime(event: Event): { label: string; sortOrder: number } {
  if (event.event_at != null) {
    const time = formatJstTime(event.event_at)
    return { label: time.label, sortOrder: time.minutes }
  }
  if (event.time_of_day === 'pre_market') {
    return {
      label: '寄り前',
      sortOrder: MARKET_TIME_SORT_ORDER[event.country]?.pre_market ?? -1,
    }
  }
  if (event.time_of_day === 'post_market') {
    return {
      label: '引け後',
      sortOrder: MARKET_TIME_SORT_ORDER[event.country]?.post_market ?? 1440,
    }
  }
  return { label: '', sortOrder: 2880 }
}

function matchesEventPeriod(reference: EventReference, event: Event): boolean {
  if (
    event.fiscal_period == null ||
    reference.target_period_end !== event.fiscal_period
  ) {
    return false
  }
  if (event.country === 'US' && event.category === 'earnings') {
    return (
      reference.kind === 'individual_document' &&
      (reference.target_period_type === '10-Q' ||
        reference.target_period_type === '10-K')
    )
  }
  return (
    event.fiscal_period_type != null &&
    reference.target_period_type === event.fiscal_period_type
  )
}

function buildEarningsStatus(
  event: Event,
  today: string,
): CalendarEarningsStatus {
  const references = event.references
  const matchingSummaryReference = references.some(
    (reference) =>
      reference.kind === 'earnings_summary' &&
      matchesEventPeriod(reference, event),
  )
  const hasSummary =
    event.has_matching_earnings_summary || matchingSummaryReference
  const individualDocuments = references.filter(
    (reference) => reference.kind === 'individual_document',
  )
  const hasMatchingIndividualDocument = individualDocuments.some((reference) =>
    matchesEventPeriod(reference, event),
  )
  const hasOfficialListing = references.some(
    (reference) => reference.kind === 'official_listing',
  )
  const label = hasSummary
    ? '実績あり'
    : hasMatchingIndividualDocument
      ? '資料あり / 実績未取得'
      : event.event_date >= today
        ? '発表予定'
        : hasOfficialListing || individualDocuments.length > 0
          ? '個別資料未確認'
          : '実績・資料未取得 / 公表未確認'
  const periodType = toFinancialPeriodType(event.fiscal_period_type)
  const internalTarget =
    event.has_matching_earnings_summary &&
    event.country === 'JP' &&
    event.stock_id != null &&
    event.fiscal_period != null &&
    periodType != null
      ? {
          stockId: event.stock_id,
          periodEnd: event.fiscal_period,
          periodType,
        }
      : undefined

  return {
    label,
    ...(internalTarget == null ? {} : { internalTarget }),
    references: references.map(toEarningsReferenceView),
  }
}

function toFinancialPeriodType(
  value: string | null | undefined,
): FinancialPeriodType | undefined {
  return value === 'FY' ||
    value === '1Q' ||
    value === '2Q' ||
    value === '3Q' ||
    value === '4Q'
    ? value
    : undefined
}

function toEarningsReferenceView(
  reference: EventReference,
): CalendarEarningsReferenceView {
  return {
    key: `${reference.kind}:${reference.source}:${reference.external_id}`,
    title: reference.title,
    source: reference.source,
    checkedAt: reference.checked_at,
    ...(reference.target_period_end == null
      ? {}
      : { targetPeriodEnd: reference.target_period_end }),
    ...(reference.target_period_type == null
      ? {}
      : { targetPeriodType: reference.target_period_type }),
    ...(reference.url == null ? {} : { href: reference.url }),
    ...(reference.publication_evidence == null
      ? {}
      : { publicationEvidence: reference.publication_evidence }),
    ...(reference.publication_evidence_url == null
      ? {}
      : { publicationEvidenceHref: reference.publication_evidence_url }),
  }
}

function isEmphasized(event: Event): boolean {
  if (event.category === 'central_bank') return true
  if (
    event.category !== 'indicator' ||
    event.country !== 'US' ||
    event.source !== 'fred'
  ) {
    return false
  }
  const releaseId = event.external_id.split(':', 1)[0] ?? ''
  return EMPHASIZED_FRED_RELEASE_IDS.has(releaseId)
}

function buildEventRow(
  event: Event,
  selectedStrategyId: string | undefined,
  today: string,
): { row: CalendarRowView; sortOrder: number } {
  const time = getEventTime(event)
  const stockId = event.stock_id ?? undefined
  const isEarningsStock = event.category === 'earnings' && stockId != null
  const title =
    isEarningsStock && event.country === 'JP'
      ? `${event.title} (${stockId})`
      : event.title

  return {
    row: {
      key: event.external_id,
      time: time.label,
      country: event.country,
      category: formatCalendarCategory(event.category),
      title,
      stockId: isEarningsStock ? stockId : undefined,
      ...(isEarningsStock
        ? { earningsStatus: buildEarningsStatus(event, today) }
        : {}),
      emphasized: isEmphasized(event),
      target: selectedStrategyId != null && event.category === 'earnings',
      muted: false,
      otherEarnings: undefined,
    },
    sortOrder: time.sortOrder,
  }
}

function buildOtherEarningsRow(event: OtherEarningsSummary): {
  row: CalendarRowView
  sortOrder: number
} {
  return {
    row: {
      key: `${event.country}:${event.event_date}:other-earnings`,
      time: '',
      country: event.country,
      category: formatCalendarCategory('earnings'),
      title: `他 ${event.count.toString()} 社`,
      emphasized: false,
      target: false,
      muted: true,
      otherEarnings: { country: event.country, eventDate: event.event_date },
    },
    sortOrder: 2881,
  }
}

export function buildCalendarDays(
  events: CalendarEvent[],
  countryFilter: CalendarCountryFilter,
  selectedStrategyId: string | undefined,
  today: string,
): CalendarDayView[] {
  const dayRows = new Map<
    string,
    Array<{ row: CalendarRowView; sortOrder: number; insertionOrder: number }>
  >()

  events.forEach((event, insertionOrder) => {
    if (countryFilter !== 'all' && event.country !== countryFilter) return

    const built =
      event.kind === 'event'
        ? buildEventRow(event, selectedStrategyId, today)
        : buildOtherEarningsRow(event)
    const rows = dayRows.get(event.event_date) ?? []
    rows.push({ ...built, insertionOrder })
    dayRows.set(event.event_date, rows)
  })

  return [...dayRows.entries()]
    .sort(([leftDate], [rightDate]) => leftDate.localeCompare(rightDate))
    .map(([date, rows]) => ({
      date,
      label: formatCalendarDayLabel(date),
      rows: rows
        .sort(
          (left, right) =>
            left.sortOrder - right.sortOrder ||
            left.insertionOrder - right.insertionOrder,
        )
        .map(({ row }) => row),
    }))
}
