import {
  type CalendarCountryFilter,
  formatCalendarDayLabel,
} from '#components/calendar/calendar-date'
import type { components } from '#lib/api/schema.gen'

type CalendarEvent = components['schemas']['CalendarEventResponse']
type Event = Extract<CalendarEvent, { kind: 'event' }>
type OtherEarningsSummary = Extract<
  CalendarEvent,
  { kind: 'other_earnings_summary' }
>

type CalendarRowView = {
  key: string
  time: string
  country: string
  category: string
  title: string
  stockId?: string
  emphasized: boolean
  target: boolean
  muted: boolean
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
): { row: CalendarRowView; sortOrder: number } {
  const time = getEventTime(event)
  const stockId = event.stock_id ?? undefined
  const isDomesticEarnings =
    event.category === 'earnings' && event.country === 'JP' && stockId != null
  const title = isDomesticEarnings ? `${event.title} (${stockId})` : event.title

  return {
    row: {
      key: event.external_id,
      time: time.label,
      country: event.country,
      category: CATEGORY_LABELS[event.category] ?? event.category,
      title,
      stockId: isDomesticEarnings ? stockId : undefined,
      emphasized: isEmphasized(event),
      target: selectedStrategyId != null && event.category === 'earnings',
      muted: false,
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
      category: CATEGORY_LABELS.earnings ?? '決算',
      title: `他 ${event.count.toString()} 社`,
      emphasized: false,
      target: false,
      muted: true,
    },
    sortOrder: 2881,
  }
}

export function buildCalendarDays(
  events: CalendarEvent[],
  countryFilter: CalendarCountryFilter,
  selectedStrategyId: string | undefined,
): CalendarDayView[] {
  const dayRows = new Map<
    string,
    Array<{ row: CalendarRowView; sortOrder: number; insertionOrder: number }>
  >()

  events.forEach((event, insertionOrder) => {
    if (countryFilter !== 'all' && event.country !== countryFilter) return

    const built =
      event.kind === 'event'
        ? buildEventRow(event, selectedStrategyId)
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
