import type {
  IChartApi,
  ISeriesApi,
  MouseEventParams,
  SeriesType,
} from 'lightweight-charts'
import { useEffect } from 'react'

import {
  type ChartAnnotation,
  findNearestHumanHorizontalLineId,
  type HorizontalLinePoint,
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
  onPlaceHorizontalLine,
  onSelectAnnotation,
}: {
  chartRef: ChartRef<IChartApi>
  candlestickSeriesRef: ChartRef<ISeriesApi<SeriesType>>
  bars: Bar[]
  annotations?: ChartAnnotation[]
  horizontalLineDrawingEnabled: boolean
  onPlaceHorizontalLine?: (line: HorizontalLinePoint) => void
  onSelectAnnotation?: (id: string) => void
}) {
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

      if (onSelectAnnotation == null) return

      const nearestLineId = findNearestHumanHorizontalLineId(
        annotations ?? [],
        (price) => activeSeries.priceToCoordinate(price),
        point.y,
      )

      if (nearestLineId != null) {
        onSelectAnnotation(nearestLineId)
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
    onPlaceHorizontalLine,
    onSelectAnnotation,
  ])
}
