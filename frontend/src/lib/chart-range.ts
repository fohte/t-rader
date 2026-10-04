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

function formatDate(date: Date): string {
  const year = date.getFullYear()
  const month = String(date.getMonth() + 1).padStart(2, '0')
  const day = String(date.getDate()).padStart(2, '0')
  return `${String(year)}-${month}-${day}`
}
