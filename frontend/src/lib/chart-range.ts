import type { Timeframe } from '#components/timeframe-selector'

const LOOKBACK_DAYS: Record<Timeframe, number> = {
  '1m': 4,
  '5m': 7,
  '15m': 7,
  '1h': 30,
  '4h': 90,
  '1d': 365,
  '1w': 365 * 3,
}

const BARS_PER_TRADING_DAY: Record<Timeframe, number> = {
  '1m': 330,
  '5m': 66,
  '15m': 22,
  '1h': 5,
  '4h': 2,
  '1d': 1,
  '1w': 1 / 5,
}
const CALENDAR_DAYS_PER_TRADING_DAY = 1.5

export interface BarsRange {
  from: string
  to: string
}

/** タイムフレームに応じた API 用の取得範囲を返す */
export function getBarsRange(
  timeframe: Timeframe,
  now = new Date(),
): BarsRange {
  const fromDate = new Date(now)
  fromDate.setDate(fromDate.getDate() - LOOKBACK_DAYS[timeframe])

  if (timeframe === '1d' || timeframe === '1w') {
    return { from: formatDate(fromDate), to: formatDate(now) }
  }

  return { from: fromDate.toISOString(), to: now.toISOString() }
}

export function extendBarsRange(
  range: BarsRange,
  timeframe: Timeframe,
  additionalBars: number,
): BarsRange {
  if (!Number.isInteger(additionalBars) || additionalBars <= 0) return range

  const additionalDays = Math.ceil(
    (additionalBars / BARS_PER_TRADING_DAY[timeframe]) *
      CALENDAR_DAYS_PER_TRADING_DAY,
  )
  const fromDate = new Date(range.from)
  fromDate.setDate(fromDate.getDate() - additionalDays)

  const from =
    timeframe === '1d' || timeframe === '1w'
      ? formatDate(fromDate)
      : fromDate.toISOString()

  return { from, to: range.to }
}

function formatDate(date: Date): string {
  const year = date.getFullYear()
  const month = String(date.getMonth() + 1).padStart(2, '0')
  const day = String(date.getDate()).padStart(2, '0')
  return `${String(year)}-${month}-${day}`
}
