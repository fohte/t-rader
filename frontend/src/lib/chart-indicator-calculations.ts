import type { ChartIndicatorConfig } from '#lib/chart-indicator-settings'

export interface ChartIndicatorBar<Time> {
  time: Time
  close: number
}

interface ChartIndicatorPoint<Time> {
  time: Time
  value: number
}

interface BollingerBands<Time> {
  middle: ChartIndicatorPoint<Time>[]
  upper: ChartIndicatorPoint<Time>[]
  lower: ChartIndicatorPoint<Time>[]
}

export interface ChartIndicatorLine<Time> {
  variant: 'main' | 'band' | 'signal' | 'histogram'
  points: ChartIndicatorPoint<Time>[]
}

function isValidPeriod(period: number): boolean {
  return Number.isInteger(period) && period > 0
}

function calculateSimpleMovingAverage<Time>(
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

function calculateExponentialMovingAverage<Time>(
  bars: readonly ChartIndicatorBar<Time>[],
  period: number,
): ChartIndicatorPoint<Time>[] {
  const values = calculateExponentialMovingAverageValues(bars, period)
  return bars.flatMap((bar, index) => {
    const value = values[index]
    return value == null ? [] : [{ time: bar.time, value }]
  })
}

function calculateExponentialMovingAverageValues<Time>(
  bars: readonly ChartIndicatorBar<Time>[],
  period: number,
): (number | undefined)[] {
  const values: (number | undefined)[] = Array.from(
    { length: bars.length },
    () => undefined,
  )
  if (!isValidPeriod(period) || bars.length < period) return values

  const alpha = 2 / (period + 1)
  let average = 0

  for (let index = 0; index < period; index += 1) {
    const bar = bars[index]
    if (bar != null) average += bar.close
  }

  average /= period
  values[period - 1] = average

  for (let index = period; index < bars.length; index += 1) {
    const bar = bars[index]
    if (bar == null) continue

    average += alpha * (bar.close - average)
    values[index] = average
  }

  return values
}

function calculateRelativeStrengthIndex<Time>(
  bars: readonly ChartIndicatorBar<Time>[],
  period: number,
): ChartIndicatorPoint<Time>[] {
  if (!isValidPeriod(period) || bars.length <= period) return []

  let averageGain = 0
  let averageLoss = 0

  for (let index = 1; index <= period; index += 1) {
    const previousBar = bars[index - 1]
    const currentBar = bars[index]
    if (previousBar == null || currentBar == null) continue

    const change = currentBar.close - previousBar.close
    averageGain += Math.max(change, 0)
    averageLoss += Math.max(-change, 0)
  }

  averageGain /= period
  averageLoss /= period

  const points: ChartIndicatorPoint<Time>[] = []
  const firstRsiBar = bars[period]
  if (firstRsiBar != null) {
    points.push({
      time: firstRsiBar.time,
      value: calculateRelativeStrengthIndexValue(averageGain, averageLoss),
    })
  }

  for (let index = period + 1; index < bars.length; index += 1) {
    const previousBar = bars[index - 1]
    const currentBar = bars[index]
    if (previousBar == null || currentBar == null) continue

    const change = currentBar.close - previousBar.close
    averageGain = (averageGain * (period - 1) + Math.max(change, 0)) / period
    averageLoss = (averageLoss * (period - 1) + Math.max(-change, 0)) / period
    points.push({
      time: currentBar.time,
      value: calculateRelativeStrengthIndexValue(averageGain, averageLoss),
    })
  }

  return points
}

function calculateRelativeStrengthIndexValue(
  averageGain: number,
  averageLoss: number,
): number {
  if (averageGain === 0 && averageLoss === 0) return 50
  if (averageLoss === 0) return 100
  if (averageGain === 0) return 0

  const relativeStrength = averageGain / averageLoss
  return 100 - 100 / (1 + relativeStrength)
}

function calculateMovingAverageConvergenceDivergence<Time>(
  bars: readonly ChartIndicatorBar<Time>[],
  fastPeriod: number,
  slowPeriod: number,
  signalPeriod: number,
): ChartIndicatorLine<Time>[] {
  const emptyLines: ChartIndicatorLine<Time>[] = [
    { variant: 'main', points: [] },
    { variant: 'signal', points: [] },
    { variant: 'histogram', points: [] },
  ]
  if (
    !isValidPeriod(fastPeriod) ||
    !isValidPeriod(slowPeriod) ||
    !isValidPeriod(signalPeriod) ||
    fastPeriod >= slowPeriod
  ) {
    return emptyLines
  }

  const fastValues = calculateExponentialMovingAverageValues(bars, fastPeriod)
  const slowValues = calculateExponentialMovingAverageValues(bars, slowPeriod)
  const macdPoints = bars.flatMap((bar, index) => {
    const fastValue = fastValues[index]
    const slowValue = slowValues[index]
    return fastValue == null || slowValue == null
      ? []
      : [{ time: bar.time, value: fastValue - slowValue }]
  })
  const signalBars = macdPoints.map(({ time, value }) => ({
    time,
    close: value,
  }))
  const signalPoints = calculateExponentialMovingAverage(
    signalBars,
    signalPeriod,
  )
  const histogramPoints = signalPoints.flatMap((signalPoint, index) => {
    const macdPoint = macdPoints[index + signalPeriod - 1]
    return macdPoint == null
      ? []
      : [
          {
            time: signalPoint.time,
            value: macdPoint.value - signalPoint.value,
          },
        ]
  })

  return [
    { variant: 'main', points: macdPoints },
    { variant: 'signal', points: signalPoints },
    { variant: 'histogram', points: histogramPoints },
  ]
}

function calculateBollingerBands<Time>(
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

export function calculateChartIndicatorLines<Time>(
  indicator: ChartIndicatorConfig,
  bars: readonly ChartIndicatorBar<Time>[],
): ChartIndicatorLine<Time>[] {
  switch (indicator.kind) {
    case 'sma':
      return [
        {
          variant: 'main',
          points: calculateSimpleMovingAverage(bars, indicator.period),
        },
      ]
    case 'ema':
      return [
        {
          variant: 'main',
          points: calculateExponentialMovingAverage(bars, indicator.period),
        },
      ]
    case 'bollinger': {
      const bands = calculateBollingerBands(
        bars,
        indicator.period,
        indicator.deviation,
      )
      return [
        { variant: 'main', points: bands.middle },
        { variant: 'band', points: bands.upper },
        { variant: 'band', points: bands.lower },
      ]
    }
    case 'rsi':
      return [
        {
          variant: 'main',
          points: calculateRelativeStrengthIndex(bars, indicator.period),
        },
      ]
    case 'macd':
      return calculateMovingAverageConvergenceDivergence(
        bars,
        indicator.fastPeriod,
        indicator.slowPeriod,
        indicator.signalPeriod,
      )
  }
}
