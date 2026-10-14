import type { components } from '#lib/api/schema.gen'

export type PortfolioHistoryPoint =
  components['schemas']['PortfolioHistoryPointResponse']

export type PortfolioHistoryPeriod = 'day' | 'week' | 'month' | 'year'

export interface PortfolioHistoryPointWithChange {
  point: PortfolioHistoryPoint
  change: number | null
}

const MAX_HISTORY_DAYS = 3_660

export function getPortfolioHistoryStartDate(
  firstTradeDate: string,
  today: string,
): string {
  const latestStart = new Date(`${today}T00:00:00Z`)
  latestStart.setUTCDate(latestStart.getUTCDate() - (MAX_HISTORY_DAYS - 1))
  const latestStartDate = latestStart.toISOString().slice(0, 10)
  return firstTradeDate < latestStartDate ? latestStartDate : firstTradeDate
}

export function addPortfolioHistoryChanges(
  points: PortfolioHistoryPoint[],
): PortfolioHistoryPointWithChange[] {
  let previousTotal: number | undefined

  return points.map((point) => {
    const change =
      previousTotal == null ? null : point.total_assets - previousTotal
    previousTotal = point.total_assets
    return { point, change }
  })
}

function getPeriodKey(
  date: string,
  period: Exclude<PortfolioHistoryPeriod, 'day'>,
) {
  if (period === 'month') return date.slice(0, 7)
  if (period === 'year') return date.slice(0, 4)

  const weekStart = new Date(`${date}T00:00:00Z`)
  const daysSinceMonday = (weekStart.getUTCDay() + 6) % 7
  weekStart.setUTCDate(weekStart.getUTCDate() - daysSinceMonday)
  return weekStart.toISOString().slice(0, 10)
}

export function selectPortfolioHistoryPoints(
  points: PortfolioHistoryPoint[],
  period: PortfolioHistoryPeriod,
): PortfolioHistoryPoint[] {
  if (period === 'day') return points

  const lastPointByPeriod = new Map<string, PortfolioHistoryPoint>()
  for (const point of points) {
    lastPointByPeriod.set(getPeriodKey(point.date, period), point)
  }

  return [...lastPointByPeriod.values()]
}
