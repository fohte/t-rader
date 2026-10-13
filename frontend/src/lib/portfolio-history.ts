import type { components } from '#lib/api/schema.gen'

export type PortfolioHistoryPoint =
  components['schemas']['PortfolioHistoryPointResponse']

export type PortfolioHistoryPeriod = 'day' | 'week' | 'month' | 'year'

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
