import {
  type IChartApi,
  type IPriceLine,
  type ISeriesApi,
  LineSeries,
  LineStyle,
  type SeriesType,
} from 'lightweight-charts'
import { useEffect, useMemo, useState } from 'react'

import {
  type AnnotationBandPeriod,
  bucketAnnotationPeriod,
  bucketAnnotationTimestamp,
  type ChartAnnotation,
  clipAnnotationBandSegment,
  getAnnotationPeriodLineRange,
  type HorizontalLinePoint,
  isAnnotationBandAnnotation,
  isHumanHorizontalLine,
  isHumanTrendLine,
  type TrendLinePoint,
} from '#lib/annotation-chart-utils'
import type { components } from '#lib/api/schema.gen'
import { observeChartTheme } from '#lib/chart-theme'
import {
  getChartBarDate,
  toCandlestickData,
  toChartTimestamp,
} from '#lib/chart-utils'
import { getVisibleTrendLine } from '#lib/trend-line-chart-utils'

type Bar = components['schemas']['Bar']
type BucketedAnnotation =
  | {
      id: string
      time: Exclude<ReturnType<typeof bucketAnnotationTimestamp>, null>
    }
  | {
      id: string
      period: NonNullable<ReturnType<typeof bucketAnnotationPeriod>>
    }

interface BandLayout {
  width: number
  bottom: number
}

interface DateMarkerLayout {
  date: string
  x: number
  bottom: number
}

interface ChartRef<T> {
  current: T | null
}

export function getAnnotationSelectionColor(): string {
  // Lightweight Charts は canvas 描画のため、CSS token を色値に解決して渡す。
  if (typeof document === 'undefined') return 'var(--muted-foreground)'

  return getComputedStyle(document.documentElement)
    .getPropertyValue('--muted-foreground')
    .trim()
}

export function useAnnotationOverlays({
  chartRef,
  candlestickSeriesRef,
  annotationPriceSeriesRef,
  trendLinePreviewSeriesRef,
  priceLineRef,
  bars,
  annotations,
  selectedAnnotationId,
  pendingHorizontalLine,
  trendLineDrawingEnabled,
  pendingTrendLineStart,
  pendingTrendLinePreviewPoint,
  highlightedTimestamp,
}: {
  chartRef: ChartRef<IChartApi>
  candlestickSeriesRef: ChartRef<ISeriesApi<SeriesType>>
  annotationPriceSeriesRef: ChartRef<ISeriesApi<'Line'>>
  trendLinePreviewSeriesRef: ChartRef<ISeriesApi<'Line'>>
  priceLineRef: ChartRef<IPriceLine>
  bars: Bar[]
  annotations?: ChartAnnotation[]
  selectedAnnotationId?: string | null
  pendingHorizontalLine?: HorizontalLinePoint | null
  trendLineDrawingEnabled: boolean
  pendingTrendLineStart?: TrendLinePoint | null
  pendingTrendLinePreviewPoint?: TrendLinePoint | null
  highlightedTimestamp?: string
}) {
  const [bandLayout, setBandLayout] = useState<BandLayout | null>(null)
  const [dateMarkerLayout, setDateMarkerLayout] =
    useState<DateMarkerLayout | null>(null)
  const [bandMarkers, setBandMarkers] = useState<{ id: string; x: number }[]>(
    [],
  )
  const [bandPeriods, setBandPeriods] = useState<AnnotationBandPeriod[]>([])
  const [selectionColor, setSelectionColor] = useState(
    getAnnotationSelectionColor,
  )
  const bandAnnotations = useMemo(() => {
    const allAnnotations = annotations ?? []
    const fallbackTrendLines = allAnnotations.flatMap<ChartAnnotation>(
      (annotation) => {
        if (!isHumanTrendLine(annotation)) return []

        const visibleTrendLine = getVisibleTrendLine(
          {
            timestamp: annotation.timestamp_start,
            price: annotation.price_start,
          },
          { timestamp: annotation.timestamp, price: annotation.price },
          bars,
        )
        if (visibleTrendLine == null || !visibleTrendLine.isCollapsed) return []

        const bar = bars.find(
          (item) =>
            toChartTimestamp(item.timestamp) === visibleTrendLine.data[0].time,
        )
        return bar == null
          ? []
          : [{ ...annotation, timestamp_start: null, timestamp: bar.timestamp }]
      },
    )

    return [
      ...allAnnotations.filter(isAnnotationBandAnnotation),
      ...fallbackTrendLines,
    ]
  }, [annotations, bars])
  const hasAnnotations = bandAnnotations.length > 0

  useEffect(() => {
    const updateSelectionColor = () => {
      setSelectionColor(getAnnotationSelectionColor())
    }
    updateSelectionColor()
    return observeChartTheme(updateSelectionColor)
  }, [])

  useEffect(() => {
    annotationPriceSeriesRef.current?.applyOptions({ color: selectionColor })
  }, [annotationPriceSeriesRef, selectionColor])

  useEffect(() => {
    const chart = chartRef.current
    const series = candlestickSeriesRef.current
    if (chart == null || series == null) return

    const lines: IPriceLine[] = []

    for (const annotation of annotations ?? []) {
      if (!isHumanHorizontalLine(annotation)) continue

      lines.push(
        series.createPriceLine({
          price: annotation.price,
          color: '#ffffff',
          lineWidth: 2,
          lineStyle: LineStyle.Solid,
          lineVisible: true,
          axisLabelVisible: true,
          title: annotation.target_kind,
          axisLabelColor: '#ffffff',
          axisLabelTextColor: '#111827',
        }),
      )
    }

    return () => {
      if (chartRef.current !== chart) return

      for (const line of lines) series.removePriceLine(line)
    }
  }, [annotations, candlestickSeriesRef, chartRef])

  useEffect(() => {
    const chart = chartRef.current
    if (chart == null) return

    const lineSeries: ISeriesApi<'Line'>[] = []
    for (const annotation of annotations ?? []) {
      if (!isHumanTrendLine(annotation)) continue

      const visibleTrendLine = getVisibleTrendLine(
        {
          timestamp: annotation.timestamp_start,
          price: annotation.price_start,
        },
        { timestamp: annotation.timestamp, price: annotation.price },
        bars,
      )
      if (visibleTrendLine == null || visibleTrendLine.isCollapsed) continue

      const series = chart.addSeries(LineSeries, {
        color: '#ffffff',
        lineWidth: 2,
        lineStyle: LineStyle.Solid,
        priceScaleId: 'right',
        priceLineVisible: false,
        lastValueVisible: false,
        crosshairMarkerVisible: false,
        pointMarkersVisible: false,
        autoscaleInfoProvider: () => null,
      })
      series.setData(visibleTrendLine.data)
      lineSeries.push(series)
    }

    return () => {
      if (chartRef.current !== chart) return

      for (const series of lineSeries) chart.removeSeries(series)
    }
  }, [annotations, bars, chartRef])

  useEffect(() => {
    const chart = chartRef.current
    const series = trendLinePreviewSeriesRef.current
    if (chart == null || series == null) return

    const previewPoint = pendingTrendLinePreviewPoint
    if (
      !trendLineDrawingEnabled ||
      pendingTrendLineStart == null ||
      previewPoint == null
    ) {
      series.setData([])
      return
    }

    const visibleTrendLine = getVisibleTrendLine(
      pendingTrendLineStart,
      previewPoint,
      bars,
    )
    series.setData(
      visibleTrendLine == null || visibleTrendLine.isCollapsed
        ? []
        : visibleTrendLine.data,
    )
    return () => {
      if (chartRef.current === chart) series.setData([])
    }
  }, [
    bars,
    chartRef,
    pendingTrendLinePreviewPoint,
    pendingTrendLineStart,
    trendLineDrawingEnabled,
    trendLinePreviewSeriesRef,
  ])

  useEffect(() => {
    const chart = chartRef.current
    const series = candlestickSeriesRef.current
    if (chart == null || series == null) return

    if (pendingHorizontalLine == null) return

    const line = series.createPriceLine({
      price: pendingHorizontalLine.price,
      color: '#facc15',
      lineWidth: 2,
      lineStyle: LineStyle.Dashed,
      lineVisible: true,
      axisLabelVisible: true,
      title: '未保存',
      axisLabelColor: '#facc15',
      axisLabelTextColor: '#111827',
    })
    return () => {
      if (chartRef.current !== chart) return

      series.removePriceLine(line)
    }
  }, [candlestickSeriesRef, chartRef, pendingHorizontalLine])

  useEffect(() => {
    const chart = chartRef.current
    const selectedBar = bars.find(
      (bar) => bar.timestamp === highlightedTimestamp,
    )
    const targetTime =
      selectedBar == null ? null : toCandlestickData([selectedBar])[0]?.time
    const date =
      selectedBar == null ? null : getChartBarDate(selectedBar.timestamp)

    if (chart == null || targetTime == null || date == null) {
      setDateMarkerLayout(null)
      return
    }

    const timeScale = chart.timeScale()
    const updateMarker = () => {
      const width = timeScale.width()
      const x = timeScale.timeToCoordinate(targetTime)
      if (x == null || x < 0 || x > width) {
        setDateMarkerLayout(null)
        return
      }

      setDateMarkerLayout({ date, x, bottom: timeScale.height() })
    }

    updateMarker()
    timeScale.subscribeVisibleTimeRangeChange(updateMarker)
    timeScale.subscribeSizeChange(updateMarker)

    return () => {
      timeScale.unsubscribeVisibleTimeRangeChange(updateMarker)
      timeScale.unsubscribeSizeChange(updateMarker)
    }
  }, [bars, chartRef, highlightedTimestamp])

  useEffect(() => {
    const chart = chartRef.current
    if (chart == null || !hasAnnotations) {
      setBandLayout(null)
      setBandMarkers([])
      setBandPeriods([])
      return
    }

    const timeScale = chart.timeScale()
    const bucketedAnnotations = bandAnnotations.flatMap<BucketedAnnotation>(
      (annotation): BucketedAnnotation[] => {
        if (annotation.timestamp_start != null) {
          const period = bucketAnnotationPeriod(
            annotation.timestamp_start,
            annotation.timestamp,
            bars,
          )
          return period == null ? [] : [{ id: annotation.id, period }]
        }

        const time = bucketAnnotationTimestamp(annotation.timestamp, bars)
        return time == null ? [] : [{ id: annotation.id, time }]
      },
    )
    const updateBand = () => {
      const width = timeScale.width()
      setBandLayout({ width, bottom: timeScale.height() })
      const markers: { id: string; x: number }[] = []
      const periods: AnnotationBandPeriod[] = []

      for (const annotation of bucketedAnnotations) {
        if ('period' in annotation) {
          const startX = timeScale.timeToCoordinate(annotation.period.start)
          const endX = timeScale.timeToCoordinate(annotation.period.end)
          if (startX == null || endX == null) continue

          const segment = clipAnnotationBandSegment(startX, endX, width)
          if (segment != null) {
            periods.push({ id: annotation.id, ...segment })
          }
          continue
        }

        const x = timeScale.timeToCoordinate(annotation.time)
        if (x != null && x >= 0 && x <= width)
          markers.push({ id: annotation.id, x })
      }

      setBandMarkers(markers)
      setBandPeriods(periods)
    }

    updateBand()
    timeScale.subscribeVisibleTimeRangeChange(updateBand)
    timeScale.subscribeSizeChange(updateBand)

    return () => {
      timeScale.unsubscribeVisibleTimeRangeChange(updateBand)
      timeScale.unsubscribeSizeChange(updateBand)
    }
  }, [bandAnnotations, bars, chartRef, hasAnnotations])

  useEffect(() => {
    const lineSeries = annotationPriceSeriesRef.current
    const series = candlestickSeriesRef.current
    if (lineSeries == null || series == null) return

    lineSeries.setData([])

    const selectedAnnotation = annotations?.find(
      (annotation) => annotation.id === selectedAnnotationId,
    )
    if (priceLineRef.current != null) {
      series.removePriceLine(priceLineRef.current)
      priceLineRef.current = null
    }

    if (
      selectedAnnotation == null ||
      selectedAnnotation.price == null ||
      isHumanHorizontalLine(selectedAnnotation) ||
      isHumanTrendLine(selectedAnnotation)
    ) {
      return
    }

    const timestampStart = selectedAnnotation.timestamp_start
    const isPeriod = timestampStart != null
    const bucketedPeriod = isPeriod
      ? bucketAnnotationPeriod(
          timestampStart,
          selectedAnnotation.timestamp,
          bars,
        )
      : null
    const periodLineRange = isPeriod
      ? getAnnotationPeriodLineRange(
          timestampStart,
          selectedAnnotation.timestamp,
          bars,
        )
      : null
    if (isPeriod && bucketedPeriod == null) return

    if (periodLineRange != null) {
      lineSeries.setData([
        { time: periodLineRange.start, value: selectedAnnotation.price },
        { time: periodLineRange.end, value: selectedAnnotation.price },
      ])
    }

    priceLineRef.current = series.createPriceLine({
      price: selectedAnnotation.price,
      color: selectionColor,
      lineWidth: 2,
      lineStyle: LineStyle.Dashed,
      lineVisible: !isPeriod,
      axisLabelVisible: true,
      title: selectedAnnotation.target_kind,
      axisLabelColor: selectionColor,
      axisLabelTextColor: '#ffffff',
    })

    return () => {
      if (priceLineRef.current != null) {
        series.removePriceLine(priceLineRef.current)
        priceLineRef.current = null
      }
    }
  }, [
    annotationPriceSeriesRef,
    annotations,
    bars,
    candlestickSeriesRef,
    priceLineRef,
    selectionColor,
    selectedAnnotationId,
  ])

  return { bandLayout, bandMarkers, bandPeriods, dateMarkerLayout }
}
