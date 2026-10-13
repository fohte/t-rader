import type { components } from '#lib/api/schema.gen'

export type IndicatorObservation =
  components['schemas']['IndicatorObservationResponse']
export type IndicatorPeriod = '1m' | '3m' | '1y' | 'all'

const PERIOD_DAYS: Record<Exclude<IndicatorPeriod, 'all'>, number> = {
  '1m': 30,
  '3m': 90,
  '1y': 365,
}

export interface IndicatorObservationSummary {
  latest: IndicatorObservation
  previous: IndicatorObservation | null
  change: number | null
  changePercent: number | null
}

export function selectIndicatorObservations(
  observations: IndicatorObservation[],
  period: IndicatorPeriod,
  now = new Date(),
): IndicatorObservation[] {
  if (period === 'all') return observations

  const from = new Date(
    Date.UTC(now.getUTCFullYear(), now.getUTCMonth(), now.getUTCDate()),
  )
  from.setUTCDate(from.getUTCDate() - PERIOD_DAYS[period])
  const fromDate = from.toISOString().slice(0, 10)

  return observations.filter((observation) => observation.date >= fromDate)
}

export function getIndicatorObservationSummary(
  observations: IndicatorObservation[],
): IndicatorObservationSummary | null {
  const latest = observations.at(-1)
  if (latest == null) return null

  const previous = observations.at(-2) ?? null
  if (previous == null) {
    return { latest, previous, change: null, changePercent: null }
  }

  const change = latest.value - previous.value
  return {
    latest,
    previous,
    change,
    changePercent:
      previous.value === 0 ? null : (change / Math.abs(previous.value)) * 100,
  }
}
