export interface ChartIndicatorBar<Time> {
  time: Time
  close: number
}

export interface ChartIndicatorPoint<Time> {
  time: Time
  value: number
}

export interface BollingerBands<Time> {
  middle: ChartIndicatorPoint<Time>[]
  upper: ChartIndicatorPoint<Time>[]
  lower: ChartIndicatorPoint<Time>[]
}

function isValidPeriod(period: number): boolean {
  return Number.isInteger(period) && period > 0
}

export function calculateSimpleMovingAverage<Time>(
  bars: readonly ChartIndicatorBar<Time>[],
  period: number,
): ChartIndicatorPoint<Time>[] {
  if (!isValidPeriod(period)) return []

  const points: ChartIndicatorPoint<Time>[] = []
  let sum = 0

  for (const [index, bar] of bars.entries()) {
    sum += bar.close

    if (index >= period) {
      const expiredBar = bars[index - period]
      if (expiredBar != null) sum -= expiredBar.close
    }

    if (index >= period - 1) {
      points.push({ time: bar.time, value: sum / period })
    }
  }

  return points
}

export function calculateExponentialMovingAverage<Time>(
  bars: readonly ChartIndicatorBar<Time>[],
  period: number,
): ChartIndicatorPoint<Time>[] {
  if (!isValidPeriod(period) || bars.length < period) return []

  const points: ChartIndicatorPoint<Time>[] = []
  const alpha = 2 / (period + 1)
  let average = 0

  for (let index = 0; index < period; index += 1) {
    const bar = bars[index]
    if (bar != null) average += bar.close
  }

  average /= period
  const seedBar = bars[period - 1]
  if (seedBar != null) points.push({ time: seedBar.time, value: average })

  for (let index = period; index < bars.length; index += 1) {
    const bar = bars[index]
    if (bar == null) continue

    average += alpha * (bar.close - average)
    points.push({ time: bar.time, value: average })
  }

  return points
}

export function calculateBollingerBands<Time>(
  bars: readonly ChartIndicatorBar<Time>[],
  period: number,
  deviationMultiplier: number,
): BollingerBands<Time> {
  const emptyBands: BollingerBands<Time> = {
    middle: [],
    upper: [],
    lower: [],
  }

  if (
    !isValidPeriod(period) ||
    !Number.isFinite(deviationMultiplier) ||
    deviationMultiplier < 0
  ) {
    return emptyBands
  }

  const middle = calculateSimpleMovingAverage(bars, period)
  const upper: ChartIndicatorPoint<Time>[] = []
  const lower: ChartIndicatorPoint<Time>[] = []

  for (let pointIndex = 0; pointIndex < middle.length; pointIndex += 1) {
    const point = middle[pointIndex]
    if (point == null) continue

    const windowStart = pointIndex
    let squaredDifferenceSum = 0

    for (let offset = 0; offset < period; offset += 1) {
      const bar = bars[windowStart + offset]
      if (bar != null) {
        const difference = bar.close - point.value
        squaredDifferenceSum += difference * difference
      }
    }

    const standardDeviation = Math.sqrt(squaredDifferenceSum / period)
    const distance = standardDeviation * deviationMultiplier
    upper.push({ time: point.time, value: point.value + distance })
    lower.push({ time: point.time, value: point.value - distance })
  }

  return { middle, upper, lower }
}
