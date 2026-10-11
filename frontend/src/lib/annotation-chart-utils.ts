import type { UTCTimestamp } from 'lightweight-charts'

import type { components } from '#lib/api/schema.gen'

type Bar = components['schemas']['Bar']
type Annotation = components['schemas']['Annotation']

export type ChartAnnotation = Pick<
  Annotation,
  | 'id'
  | 'price'
  | 'status'
  | 'target_kind'
  | 'text'
  | 'timestamp'
  | 'timestamp_start'
>

export interface AnnotationBandMarker {
  id: string
  x: number
}

export interface AnnotationBandCluster {
  markers: AnnotationBandMarker[]
  x: number
}

export interface AnnotationBandPeriod {
  id: string
  startX: number
  endX: number
}

export interface AnnotationBandSegment {
  startX: number
  endX: number
}

interface AnnotationBandTargetGroup {
  kind: 'marker' | 'period'
  ids: string[]
  startX: number
  endX: number
  centerX: number
  markers: AnnotationBandMarker[]
  periods: AnnotationBandPeriod[]
}

export interface AnnotationBandTargetGroups {
  markers: AnnotationBandTargetGroup[]
  periods: AnnotationBandTargetGroup[]
}

interface BarTimePoint {
  time: UTCTimestamp
  date: string
}

const tokyoDateFormatter = new Intl.DateTimeFormat('en-US', {
  timeZone: 'Asia/Tokyo',
  year: 'numeric',
  month: '2-digit',
  day: '2-digit',
})

function getTokyoDate(value: string): string | null {
  // 日足のバーと同じ日本時間の日付で、保存時刻を表示用の足へ割り当てる。
  const date = new Date(value)
  if (!Number.isFinite(date.getTime())) return null

  const parts = tokyoDateFormatter.formatToParts(date)
  const year = parts.find((part) => part.type === 'year')?.value
  const month = parts.find((part) => part.type === 'month')?.value
  const day = parts.find((part) => part.type === 'day')?.value
  if (year == null || month == null || day == null) return null

  return `${year}-${month}-${day}`
}

function toUTCTimestamp(value: string): UTCTimestamp | null {
  const seconds = Math.floor(new Date(value).getTime() / 1000)
  if (!Number.isFinite(seconds)) return null

  // eslint-disable-next-line @typescript-eslint/no-unsafe-type-assertion -- UTCTimestamp はブランド型
  return seconds as UTCTimestamp
}

function getIntradayIntervalSeconds(timeframe: string): number | null {
  const match = /^(\d+)(s|m|h)$/.exec(timeframe)
  if (match == null) return null

  const value = Number(match[1])
  const unit = match[2]
  if (unit === 's') return value
  if (unit === 'm') return value * 60
  return value * 60 * 60
}

function getBarTimePoints(bars: Bar[]): BarTimePoint[] {
  return bars
    .map((bar) => ({
      time: toUTCTimestamp(bar.timestamp),
      date: getTokyoDate(bar.timestamp),
    }))
    .filter(
      (point): point is BarTimePoint =>
        point.time != null && point.date != null,
    )
    .sort((left, right) => left.time - right.time)
}

function bucketTimestamp(
  timestamp: string,
  points: BarTimePoint[],
  timeframe: string | undefined,
  clampToBars: boolean,
): UTCTimestamp | null {
  const annotationTime = toUTCTimestamp(timestamp)
  const first = points[0]
  const last = points.at(-1)
  if (annotationTime == null || first == null || last == null) return null

  const intradayInterval =
    timeframe == null ? null : getIntradayIntervalSeconds(timeframe)

  if (intradayInterval != null) {
    if (
      !clampToBars &&
      (annotationTime < first.time ||
        annotationTime > last.time + intradayInterval)
    ) {
      return null
    }

    return (
      points.findLast((point) => point.time <= annotationTime)?.time ??
      (clampToBars ? first.time : null)
    )
  }

  const annotationDate = getTokyoDate(timestamp)
  if (annotationDate == null) return null
  if (
    !clampToBars &&
    (annotationDate < first.date || annotationDate > last.date)
  ) {
    return null
  }

  return (
    points.findLast((point) => point.date <= annotationDate)?.time ??
    (clampToBars ? first.time : null)
  )
}

/** 保存時刻を変更せず、表示中のバーの時刻へ割り当てる。 */
export function bucketAnnotationTimestamp(
  timestamp: string,
  bars: Bar[],
): UTCTimestamp | null {
  if (bars.length === 0) return null

  return bucketTimestamp(
    timestamp,
    getBarTimePoints(bars),
    bars[0]?.timeframe,
    false,
  )
}

/** 期間の両端を同じ足へ割り当て、バー範囲と重なる部分を返す。 */
export function bucketAnnotationPeriod(
  timestampStart: string,
  timestampEnd: string,
  bars: Bar[],
): { start: UTCTimestamp; end: UTCTimestamp } | null {
  if (bars.length === 0) return null

  const points = getBarTimePoints(bars)
  const first = points[0]
  const last = points.at(-1)
  const startTime = toUTCTimestamp(timestampStart)
  const endTime = toUTCTimestamp(timestampEnd)
  const startDate = getTokyoDate(timestampStart)
  const endDate = getTokyoDate(timestampEnd)
  if (
    first == null ||
    last == null ||
    startTime == null ||
    endTime == null ||
    startDate == null ||
    endDate == null ||
    startTime > endTime
  ) {
    return null
  }

  const timeframe = bars[0]?.timeframe
  const intradayInterval =
    timeframe == null ? null : getIntradayIntervalSeconds(timeframe)
  if (intradayInterval != null) {
    if (endTime < first.time || startTime > last.time + intradayInterval) {
      return null
    }
  } else if (endDate < first.date || startDate > last.date) {
    return null
  }

  const start = bucketTimestamp(timestampStart, points, timeframe, true)
  const end = bucketTimestamp(timestampEnd, points, timeframe, true)
  if (start == null || end == null || start > end) return null

  return { start, end }
}

/** 価格線が描けるよう、同じ足に丸められた期間を隣の足まで広げる。 */
export function getAnnotationPeriodLineRange(
  timestampStart: string,
  timestampEnd: string,
  bars: Bar[],
): { start: UTCTimestamp; end: UTCTimestamp } | null {
  const period = bucketAnnotationPeriod(timestampStart, timestampEnd, bars)
  if (period == null || period.start !== period.end) return period

  const points = getBarTimePoints(bars)
  const adjacentPoint =
    points.find((point) => point.time > period.start) ??
    points.findLast((point) => point.time < period.start)
  if (adjacentPoint == null) return null

  return adjacentPoint.time < period.start
    ? { start: adjacentPoint.time, end: period.end }
    : { start: period.start, end: adjacentPoint.time }
}

/** 表示範囲外を切り落とした期間座標を返す。 */
export function clipAnnotationBandSegment(
  startX: number,
  endX: number,
  width: number,
  minimumWidth = 6,
): AnnotationBandSegment | null {
  if (
    !Number.isFinite(startX) ||
    !Number.isFinite(endX) ||
    !Number.isFinite(width) ||
    startX > endX ||
    width <= 0 ||
    endX < 0 ||
    startX > width
  ) {
    return null
  }

  const visibleStart = Math.max(0, startX)
  const visibleEnd = Math.min(width, endX)
  if (visibleEnd - visibleStart >= minimumWidth) {
    return { startX: visibleStart, endX: visibleEnd }
  }

  const segmentWidth = Math.min(minimumWidth, width)
  const center = (visibleStart + visibleEnd) / 2
  const clippedStart = Math.max(
    0,
    Math.min(center - segmentWidth / 2, width - segmentWidth),
  )

  return { startX: clippedStart, endX: clippedStart + segmentWidth }
}

/** 近接する印をまとめ、帯に表示する座標と件数を返す。 */
export function clusterAnnotationBandMarkers(
  markers: AnnotationBandMarker[],
  minimumDistance = 16,
): AnnotationBandCluster[] {
  const sorted = [...markers].sort((left, right) => left.x - right.x)
  const groups: AnnotationBandMarker[][] = []

  for (const marker of sorted) {
    const current = groups.at(-1)
    const first = current?.[0]
    if (
      current != null &&
      first != null &&
      marker.x - first.x < minimumDistance
    ) {
      current.push(marker)
    } else {
      groups.push([marker])
    }
  }

  return groups.map((group) => ({
    markers: group,
    x: group.reduce((sum, marker) => sum + marker.x, 0) / group.length,
  }))
}

/** 点印は近接する点同士で、期間印は重なる期間同士で選択対象をまとめる。 */
export function groupAnnotationBandTargets(
  markers: AnnotationBandMarker[],
  periods: AnnotationBandPeriod[],
  minimumDistance = 16,
): AnnotationBandTargetGroups {
  const markerGroups = clusterAnnotationBandMarkers(
    markers,
    minimumDistance,
  ).map(({ markers: group, x }) => ({
    kind: 'marker' as const,
    ids: group.map((marker) => marker.id),
    startX: group[0]?.x ?? x,
    endX: group.at(-1)?.x ?? x,
    centerX: x,
    markers: group,
    periods: [],
  }))

  const periodGroups: AnnotationBandTargetGroup[] = []
  const sortedPeriods = [...periods].sort(
    (left, right) => left.startX - right.startX,
  )
  for (const period of sortedPeriods) {
    const current = periodGroups.at(-1)
    if (current != null && period.startX - current.endX < minimumDistance) {
      current.ids.push(period.id)
      current.endX = Math.max(current.endX, period.endX)
      current.centerX = (current.startX + current.endX) / 2
      current.periods.push(period)
    } else {
      periodGroups.push({
        kind: 'period',
        ids: [period.id],
        startX: period.startX,
        endX: period.endX,
        centerX: (period.startX + period.endX) / 2,
        markers: [],
        periods: [period],
      })
    }
  }

  return { markers: markerGroups, periods: periodGroups }
}
