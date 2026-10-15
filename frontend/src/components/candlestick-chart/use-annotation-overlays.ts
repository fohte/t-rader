import {
  type IChartApi,
  type IPriceLine,
  type ISeriesApi,
  LineStyle,
  type SeriesType,
} from 'lightweight-charts'
import { useEffect, useState } from 'react'

import {
  type AnnotationBandPeriod,
  bucketAnnotationPeriod,
  bucketAnnotationTimestamp,
  type ChartAnnotation,
  clipAnnotationBandSegment,
  getAnnotationPeriodLineRange,
} from '#lib/annotation-chart-utils'
import type { components } from '#lib/api/schema.gen'
import { getChartBarDate, toCandlestickData } from '#lib/chart-utils'

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

export function useAnnotationOverlays({
  chartRef,
  candlestickSeriesRef,
  annotationPriceSeriesRef,
  priceLineRef,
  bars,
  annotations,
  selectedAnnotationId,
  highlightedTimestamp,
}: {
  chartRef: ChartRef<IChartApi>
  candlestickSeriesRef: ChartRef<ISeriesApi<SeriesType>>
  annotationPriceSeriesRef: ChartRef<ISeriesApi<'Line'>>
  priceLineRef: ChartRef<IPriceLine>
  bars: Bar[]
  annotations?: ChartAnnotation[]
  selectedAnnotationId?: string | null
  highlightedTimestamp?: string
}) {
  const [bandLayout, setBandLayout] = useState<BandLayout | null>(null)
  const [dateMarkerLayout, setDateMarkerLayout] =
    useState<DateMarkerLayout | null>(null)
  const [bandMarkers, setBandMarkers] = useState<{ id: string; x: number }[]>(
    [],
  )
  const [bandPeriods, setBandPeriods] = useState<AnnotationBandPeriod[]>([])
  const hasAnnotations = (annotations?.length ?? 0) > 0

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
    const bucketedAnnotations = (annotations ?? []).flatMap<BucketedAnnotation>(
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
  }, [annotations, bars, chartRef, hasAnnotations])

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

    if (selectedAnnotation?.price == null) return

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
      color: '#ef4444',
      lineWidth: 2,
      lineStyle: LineStyle.Dashed,
      lineVisible: !isPeriod,
      axisLabelVisible: true,
      title: selectedAnnotation.target_kind,
      axisLabelColor: '#ef4444',
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
    selectedAnnotationId,
  ])

  return { bandLayout, bandMarkers, bandPeriods, dateMarkerLayout }
}
