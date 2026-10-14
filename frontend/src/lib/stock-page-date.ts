import type { components } from '#lib/api/schema.gen'
import type { BarsRange } from '#lib/chart-range'
import { getChartBarDate } from '#lib/chart-utils'

type Bar = components['schemas']['Bar']

const DATE_PATTERN = /^(\d{4})-(\d{2})-(\d{2})$/
const DATE_RANGE_RADIUS_DAYS = 182

export type StockPageSearch = { date?: string }

function parseCalendarDate(value: string): Date | null {
  const match = DATE_PATTERN.exec(value)
  if (match == null) return null

  const [, yearText, monthText, dayText] = match
  const year = Number(yearText)
  const month = Number(monthText)
  const day = Number(dayText)
  if (year < 1 || year > 9999) return null

  const date = new Date(0)
  date.setUTCHours(0, 0, 0, 0)
  date.setUTCFullYear(year, month - 1, day)
  if (
    date.getUTCFullYear() !== year ||
    date.getUTCMonth() + 1 !== month ||
    date.getUTCDate() !== day
  ) {
    return null
  }

  return date
}

function formatCalendarDate(date: Date): string {
  const year = date.getUTCFullYear().toString().padStart(4, '0')
  const month = (date.getUTCMonth() + 1).toString().padStart(2, '0')
  const day = date.getUTCDate().toString().padStart(2, '0')
  return `${year}-${month}-${day}`
}

function shiftCalendarDate(date: Date, days: number): string {
  const shifted = new Date(date)
  shifted.setUTCDate(shifted.getUTCDate() + days)

  if (shifted.getUTCFullYear() < 1) return '0001-01-01'
  if (shifted.getUTCFullYear() > 9999) return '9999-12-31'
  return formatCalendarDate(shifted)
}

export function validateStockPageSearch(
  search: Record<string, unknown>,
): StockPageSearch {
  const date = search.date
  return typeof date === 'string' && parseCalendarDate(date) != null
    ? { date }
    : {}
}

export function getDailyBarsRangeAroundDate(date: string): BarsRange | null {
  const parsed = parseCalendarDate(date)
  if (parsed == null) return null

  return {
    from: shiftCalendarDate(parsed, -DATE_RANGE_RADIUS_DAYS),
    to: shiftCalendarDate(parsed, DATE_RANGE_RADIUS_DAYS),
  }
}

export function findDailyBarForDate(bars: Bar[], date: string): Bar | null {
  if (parseCalendarDate(date) == null) return null
  return bars.find((bar) => getChartBarDate(bar.timestamp) === date) ?? null
}
