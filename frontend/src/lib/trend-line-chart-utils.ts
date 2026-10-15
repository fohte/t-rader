import type { UTCTimestamp } from 'lightweight-charts'

import {
  bucketAnnotationPeriod,
  bucketAnnotationTimestamp,
  orderTrendLinePoints,
  type TrendLinePoint,
} from '#lib/annotation-chart-utils'
import type { components } from '#lib/api/schema.gen'
import { getChartBarDate, toChartTimestamp } from '#lib/chart-utils'

type Bar = components['schemas']['Bar']

interface TrendLineSeriesPoint {
  time: UTCTimestamp
  value: number
}

export interface VisibleTrendLine {
  data: [TrendLineSeriesPoint, TrendLineSeriesPoint]
  isCollapsed: boolean
}

export function getVisibleTrendLine(
  start: TrendLinePoint,
  end: TrendLinePoint,
  bars: Bar[],
): VisibleTrendLine | null {
  const [orderedStart, orderedEnd] = orderTrendLinePoints(start, end)
  const range = getTrendLineBarRange(orderedStart, orderedEnd, bars)
  if (range == null) return null

  const startTime = bucketTrendLineTimestamp(orderedStart.timestamp, bars)
  const endTime = bucketTrendLineTimestamp(orderedEnd.timestamp, bars)
  const startPrice =
    startTime == null
      ? interpolatePrice(orderedStart, orderedEnd, range.start)
      : orderedStart.price
  const endPrice =
    endTime == null
      ? interpolatePrice(orderedStart, orderedEnd, range.end)
      : orderedEnd.price
  if (startPrice == null || endPrice == null) return null

  return {
    data: [
      { time: range.start, value: startPrice },
      { time: range.end, value: endPrice },
    ],
    isCollapsed: range.start === range.end,
  }
}

function getTrendLineBarRange(
  start: TrendLinePoint,
  end: TrendLinePoint,
  bars: Bar[],
): { start: UTCTimestamp; end: UTCTimestamp } | null {
  if (bars[0]?.timeframe !== '1w') {
    return bucketAnnotationPeriod(start.timestamp, end.timestamp, bars)
  }

  const startWeek = getWeekStartDate(start.timestamp)
  const endWeek = getWeekStartDate(end.timestamp)
  const barWeeks = getWeeklyBarPoints(bars)
  const first = barWeeks[0]
  const last = barWeeks.at(-1)
  if (
    startWeek == null ||
    endWeek == null ||
    first == null ||
    last == null ||
    endWeek < first.date ||
    startWeek > last.date
  ) {
    return null
  }

  return {
    start: bucketWeeklyTimestamp(startWeek, barWeeks) ?? first.time,
    end: bucketWeeklyTimestamp(endWeek, barWeeks) ?? last.time,
  }
}

function bucketTrendLineTimestamp(
  timestamp: string,
  bars: Bar[],
): UTCTimestamp | null {
  if (bars[0]?.timeframe !== '1w') {
    return bucketAnnotationTimestamp(timestamp, bars)
  }

  const week = getWeekStartDate(timestamp)
  if (week == null) return null

  const barWeeks = getWeeklyBarPoints(bars)
  const first = barWeeks[0]
  const last = barWeeks.at(-1)
  if (first == null || last == null || week < first.date || week > last.date) {
    return null
  }

  return bucketWeeklyTimestamp(week, barWeeks)
}

function getWeeklyBarPoints(bars: Bar[]) {
  return bars
    .flatMap((bar) => {
      const date = getWeekStartDate(bar.timestamp)
      return date == null
        ? []
        : [{ date, time: toChartTimestamp(bar.timestamp) }]
    })
    .sort((left, right) => left.time - right.time)
}

function bucketWeeklyTimestamp(
  week: string,
  barWeeks: { date: string; time: UTCTimestamp }[],
): UTCTimestamp | null {
  return barWeeks.findLast((barWeek) => barWeek.date <= week)?.time ?? null
}

function getWeekStartDate(timestamp: string): string | null {
  const date = getChartBarDate(timestamp)
  if (date == null) return null

  const day = new Date(`${date}T00:00:00Z`).getUTCDay()
  const daysSinceMonday = (day + 6) % 7
  const weekStart = new Date(`${date}T00:00:00Z`)
  weekStart.setUTCDate(weekStart.getUTCDate() - daysSinceMonday)
  return weekStart.toISOString().slice(0, 10)
}

function interpolatePrice(
  start: TrendLinePoint,
  end: TrendLinePoint,
  time: UTCTimestamp,
): number | null {
  const startTime = toChartTimestamp(start.timestamp)
  const endTime = toChartTimestamp(end.timestamp)
  const duration = endTime - startTime
  if (duration <= 0 || !Number.isFinite(duration)) return null

  const ratio = (time - startTime) / duration
  const price = start.price + ratio * (end.price - start.price)
  return Number.isFinite(price) ? price : null
}
