export type CalendarCountry = 'JP' | 'US' | 'EU'
export type CalendarCountryFilter = CalendarCountry | 'all'

export type CalendarWeekRange = {
  from: string
  to: string
}

export type CalendarSearch = {
  country?: CalendarCountry
  strategy?: string
  week?: string
}

const DATE_PATTERN = /^(\d{4})-(\d{2})-(\d{2})$/
const COUNTRY_CODES: readonly CalendarCountry[] = ['JP', 'US', 'EU']

function parseDate(value: string): Date | undefined {
  const match = DATE_PATTERN.exec(value)
  if (match == null) return undefined

  const [, yearText, monthText, dayText] = match
  const date = new Date(
    Date.UTC(Number(yearText), Number(monthText) - 1, Number(dayText)),
  )
  if (
    date.getUTCFullYear() !== Number(yearText) ||
    date.getUTCMonth() + 1 !== Number(monthText) ||
    date.getUTCDate() !== Number(dayText)
  ) {
    return undefined
  }
  return date
}

function formatDate(date: Date): string {
  const year = date.getUTCFullYear().toString().padStart(4, '0')
  const month = (date.getUTCMonth() + 1).toString().padStart(2, '0')
  const day = date.getUTCDate().toString().padStart(2, '0')
  return `${year}-${month}-${day}`
}

function shiftDate(value: string, days: number): string {
  const date = parseDate(value)
  if (date == null) return value
  date.setUTCDate(date.getUTCDate() + days)
  return formatDate(date)
}

export function getTokyoDate(now: Date = new Date()): string {
  const parts = new Intl.DateTimeFormat('en-CA', {
    timeZone: 'Asia/Tokyo',
    year: 'numeric',
    month: '2-digit',
    day: '2-digit',
  }).formatToParts(now)
  const part = (type: Intl.DateTimeFormatPartTypes) =>
    parts.find((item) => item.type === type)?.value ?? ''
  return `${part('year')}-${part('month')}-${part('day')}`
}

export function getCalendarWeekRange(dateValue: string): CalendarWeekRange {
  const date = parseDate(dateValue)
  if (date == null) return getCalendarWeekRange(getTokyoDate())

  const dayOfWeek = date.getUTCDay()
  const daysSinceMonday = (dayOfWeek + 6) % 7
  const from = shiftDate(dateValue, -daysSinceMonday)
  return { from, to: shiftDate(from, 6) }
}

export function shiftCalendarWeek(weekStart: string, weeks: number): string {
  return shiftDate(weekStart, weeks * 7)
}

export function isCalendarCountryFilter(
  value: string,
): value is CalendarCountryFilter {
  return value === 'all' || COUNTRY_CODES.some((country) => country === value)
}

export function validateCalendarSearch(
  search: Record<string, unknown>,
): CalendarSearch {
  const country = COUNTRY_CODES.find((code) => code === search.country)
  const week =
    typeof search.week === 'string' && parseDate(search.week) != null
      ? getCalendarWeekRange(search.week).from
      : undefined
  const strategy =
    typeof search.strategy === 'string' && search.strategy.length > 0
      ? search.strategy
      : undefined

  return { country, strategy, week }
}

export function formatCalendarDayLabel(dateValue: string): string {
  const date = parseDate(dateValue)
  if (date == null) return dateValue
  const weekday =
    ['日', '月', '火', '水', '木', '金', '土'][date.getUTCDay()] ?? ''
  const month = (date.getUTCMonth() + 1).toString()
  const day = date.getUTCDate().toString()
  return `${month}/${day} (${weekday})`
}

export function formatCalendarWeekRange(range: CalendarWeekRange): string {
  const from = parseDate(range.from)
  const to = parseDate(range.to)
  if (from == null || to == null) return `${range.from} – ${range.to}`

  const startYear = from.getUTCFullYear().toString()
  const endYear = to.getUTCFullYear().toString()
  const startMonth = (from.getUTCMonth() + 1).toString()
  const endMonth = (to.getUTCMonth() + 1).toString()
  const startDay = from.getUTCDate().toString()
  const endDay = to.getUTCDate().toString()
  const start = `${startYear}/${startMonth}/${startDay}`
  const end =
    from.getUTCFullYear() === to.getUTCFullYear()
      ? `${endMonth}/${endDay}`
      : `${endYear}/${endMonth}/${endDay}`
  return `${start} – ${end}`
}
