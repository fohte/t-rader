import type { components } from '#lib/api/schema.gen'
import type { BarsRange } from '#lib/chart-range'
import { getChartBarDate } from '#lib/chart-utils'

type Bar = components['schemas']['Bar']

const DATE_PATTERN = /^(\d{4})-(\d{2})-(\d{2})$/
const DATE_RANGE_RADIUS_DAYS = 182

export type StockPageTab = 'annotations' | 'notes' | 'trades' | 'financials'
export type FinancialPeriodType = 'FY' | '1Q' | '2Q' | '3Q' | '4Q'

export type StockPageSearch = {
  date?: string
  stockTab?: StockPageTab
  periodEnd?: string
  periodType?: FinancialPeriodType
}

const STOCK_PAGE_TABS = [
  'annotations',
  'notes',
  'trades',
  'financials',
] as const satisfies readonly StockPageTab[]
const FINANCIAL_PERIOD_TYPES = [
  'FY',
  '1Q',
  '2Q',
  '3Q',
  '4Q',
] as const satisfies readonly FinancialPeriodType[]

function isStockPageTab(value: string): value is StockPageTab {
  return STOCK_PAGE_TABS.some((tab) => tab === value)
}

function isFinancialPeriodType(value: string): value is FinancialPeriodType {
  return FINANCIAL_PERIOD_TYPES.some((periodType) => periodType === value)
}

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
  const stockTab = search.stockTab
  const periodEnd = search.periodEnd
  const periodType = search.periodType

  return {
    ...(typeof date === 'string' && parseCalendarDate(date) != null
      ? { date }
      : {}),
    ...(typeof stockTab === 'string' && isStockPageTab(stockTab)
      ? { stockTab }
      : {}),
    ...(typeof periodEnd === 'string' &&
    parseCalendarDate(periodEnd) != null &&
    typeof periodType === 'string' &&
    isFinancialPeriodType(periodType)
      ? {
          periodEnd,
          periodType,
        }
      : {}),
  }
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
