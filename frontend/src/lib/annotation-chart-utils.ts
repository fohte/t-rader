import type { UTCTimestamp } from 'lightweight-charts'

import type { components } from '#lib/api/schema.gen'

type Bar = components['schemas']['Bar']
type Annotation = components['schemas']['Annotation']

export type ChartAnnotation = Pick<
  Annotation,
  'id' | 'price' | 'status' | 'target_kind' | 'text' | 'timestamp'
>

export interface AnnotationBandMarker {
  id: string
  x: number
}

export interface AnnotationBandCluster {
  markers: AnnotationBandMarker[]
  x: number
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

/** 保存時刻を変更せず、表示中のバーの時刻へ割り当てる。 */
export function bucketAnnotationTimestamp(
  timestamp: string,
  bars: Bar[],
): UTCTimestamp | null {
  const annotationTime = toUTCTimestamp(timestamp)
  if (annotationTime == null || bars.length === 0) return null

  const points = bars
    .map((bar) => ({
      time: toUTCTimestamp(bar.timestamp),
      date: getTokyoDate(bar.timestamp),
    }))
    .filter(
      (point): point is { time: UTCTimestamp; date: string } =>
        point.time != null && point.date != null,
    )
    .sort((left, right) => left.time - right.time)
  const first = points[0]
  const last = points.at(-1)
  if (first == null || last == null) return null

  const timeframe = bars[0]?.timeframe
  const intradayInterval =
    timeframe == null ? null : getIntradayIntervalSeconds(timeframe)

  if (intradayInterval != null) {
    if (
      annotationTime < first.time ||
      annotationTime > last.time + intradayInterval
    ) {
      return null
    }

    return (
      points.findLast((point) => point.time <= annotationTime)?.time ?? null
    )
  }

  const annotationDate = getTokyoDate(timestamp)
  if (annotationDate == null) return null
  if (annotationDate < first.date || annotationDate > last.date) return null

  return points.findLast((point) => point.date <= annotationDate)?.time ?? null
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
