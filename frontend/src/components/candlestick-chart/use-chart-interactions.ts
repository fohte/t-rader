import type {
  IChartApi,
  ISeriesApi,
  MouseEventParams,
  SeriesType,
} from 'lightweight-charts'
import { useEffect, useState } from 'react'

import {
  bucketAnnotationTimestamp,
  type ChartAnnotation,
  findNearestHumanHorizontalLineId,
  findNearestHumanTrendLineId,
  type HorizontalLinePoint,
  type TrendLinePoint,
} from '#lib/annotation-chart-utils'
import type { components } from '#lib/api/schema.gen'
import { toChartTimestamp } from '#lib/chart-utils'

type Bar = components['schemas']['Bar']

interface ChartRef<T> {
  current: T | null
}

export function useChartInteractions({
  chartRef,
  candlestickSeriesRef,
  bars,
  annotations,
  horizontalLineDrawingEnabled,
  trendLineDrawingEnabled,
  pendingTrendLineStart,
  onPlaceHorizontalLine,
  onPlaceTrendLinePoint,
  onSelectAnnotation,
}: {
  chartRef: ChartRef<IChartApi>
  candlestickSeriesRef: ChartRef<ISeriesApi<SeriesType>>
  bars: Bar[]
  annotations?: ChartAnnotation[]
  horizontalLineDrawingEnabled: boolean
  trendLineDrawingEnabled: boolean
  pendingTrendLineStart?: TrendLinePoint | null
  onPlaceHorizontalLine?: (line: HorizontalLinePoint) => void
  onPlaceTrendLinePoint?: (point: TrendLinePoint) => void
  onSelectAnnotation?: (id: string) => void
}) {
  const [trendLineCursorPoint, setTrendLineCursorPoint] =
    useState<TrendLinePoint | null>(null)

  useEffect(() => {
    const chart = chartRef.current
    const series = candlestickSeriesRef.current
    if (chart == null || series == null) return

    function handleChartClick(param: MouseEventParams) {
      const activeChart = chartRef.current
      const activeSeries = candlestickSeriesRef.current
      if (activeChart == null || activeSeries == null) return

      const point = param.point
      if (point == null || point.x < 0 || point.y < 0) return
      if (point.x > activeChart.timeScale().width()) return

      if (horizontalLineDrawingEnabled) {
        if (onPlaceHorizontalLine == null || typeof param.time !== 'number') {
          return
        }

        const bar = bars.find(
          (item) => toChartTimestamp(item.timestamp) === param.time,
        )
        const price = activeSeries.coordinateToPrice(point.y)
        if (bar == null || price == null || !Number.isFinite(price)) return

        onPlaceHorizontalLine({ timestamp: bar.timestamp, price })
        return
      }

      if (trendLineDrawingEnabled) {
        if (onPlaceTrendLinePoint == null) return

        const point = getTrendLinePoint(param, activeSeries, bars)
        if (point == null) return

        onPlaceTrendLinePoint(point)
        setTrendLineCursorPoint(point)
        return
      }

      if (onSelectAnnotation == null) return

      const nearestLineId = findNearestHumanHorizontalLineId(
        annotations ?? [],
        (price) => activeSeries.priceToCoordinate(price),
        point.y,
      )

      if (nearestLineId != null) {
        onSelectAnnotation(nearestLineId)
        return
      }

      const nearestTrendLineId = findNearestHumanTrendLineId(
        annotations ?? [],
        (timestamp, price) => {
          const time = bucketAnnotationTimestamp(timestamp, bars)
          if (time == null) return null

          const x = activeChart.timeScale().timeToCoordinate(time)
          const y = activeSeries.priceToCoordinate(price)
          return x == null || y == null ? null : { x, y }
        },
        point.x,
        point.y,
      )

      if (nearestTrendLineId != null) {
        onSelectAnnotation(nearestTrendLineId)
      }
    }

    chart.subscribeClick(handleChartClick)
    return () => {
      chart.unsubscribeClick(handleChartClick)
    }
  }, [
    annotations,
    bars,
    horizontalLineDrawingEnabled,
    trendLineDrawingEnabled,
    pendingTrendLineStart,
    onPlaceHorizontalLine,
    onPlaceTrendLinePoint,
    onSelectAnnotation,
  ])

  useEffect(() => {
    if (!trendLineDrawingEnabled || pendingTrendLineStart == null) {
      setTrendLineCursorPoint(null)
      return
    }

    const chart = chartRef.current
    if (chart == null) return

    function handleCrosshairMove(param: MouseEventParams) {
      const activeSeries = candlestickSeriesRef.current
      setTrendLineCursorPoint(
        activeSeries == null
          ? null
          : getTrendLinePoint(param, activeSeries, bars),
      )
    }

    chart.subscribeCrosshairMove(handleCrosshairMove)
    return () => {
      chart.unsubscribeCrosshairMove(handleCrosshairMove)
    }
  }, [
    bars,
    chartRef,
    candlestickSeriesRef,
    pendingTrendLineStart,
    trendLineDrawingEnabled,
  ])

  return { trendLineCursorPoint }
}

function getTrendLinePoint(
  param: MouseEventParams,
  series: ISeriesApi<SeriesType>,
  bars: Bar[],
): TrendLinePoint | null {
  if (param.point == null || typeof param.time !== 'number') return null

  const bar = bars.find(
    (item) => toChartTimestamp(item.timestamp) === param.time,
  )
  const price = series.coordinateToPrice(param.point.y)
  if (bar == null || price == null || !Number.isFinite(price)) return null

  return { timestamp: bar.timestamp, price }
}
