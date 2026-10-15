import {
  CandlestickSeries,
  createChart,
  createSeriesMarkers,
  HistogramSeries,
  type IChartApi,
  type IPriceLine,
  type ISeriesApi,
  type ISeriesMarkersPluginApi,
  LineSeries,
  LineStyle,
  type MouseEventParams,
  type SeriesType,
  type Time,
} from 'lightweight-charts'
import { useEffect, useRef } from 'react'

import { AnnotationChartBand } from '#components/annotations/annotation-chart-band'
import { useAnnotationOverlays } from '#components/candlestick-chart/use-annotation-overlays'
import { ChartIndicatorLegend } from '#components/chart-indicator-legend'
import {
  type ChartAnnotation,
  type HorizontalLinePoint,
  isHumanHorizontalLine,
} from '#lib/annotation-chart-utils'
import type { components } from '#lib/api/schema.gen'
import { calculateChartIndicatorLines } from '#lib/chart-indicator-calculations'
import type { ChartIndicatorConfig } from '#lib/chart-indicator-settings'
import {
  getChartThemeColors,
  getChartThemeOptions,
  observeChartResize,
  observeChartTheme,
} from '#lib/chart-theme'
import {
  type ChartCurrency,
  toCandlestickData,
  toTradeMarkers,
  toVolumeData,
  type TradeMarker,
} from '#lib/chart-utils'
import { cn } from '#lib/utils'

type Bar = components['schemas']['Bar']
const EMPTY_INDICATORS: ChartIndicatorConfig[] = []

interface CandlestickChartProps {
  bars: Bar[]
  trades?: TradeMarker[]
  selectedTradeId?: string
  annotations?: ChartAnnotation[]
  indicators?: ChartIndicatorConfig[]
  displayRangeStart?: string
  onSelectAnnotation?: (id: string) => void
  selectedAnnotationId?: string | null
  horizontalLineDrawingEnabled?: boolean
  pendingHorizontalLine?: HorizontalLinePoint | null
  onPlaceHorizontalLine?: (line: HorizontalLinePoint) => void
  currency?: ChartCurrency
  priceScaleTopMargin?: number
  intraday?: boolean
  highlightedTimestamp?: string
  className?: string
}

export function CandlestickChart({
  bars,
  trades,
  selectedTradeId,
  annotations,
  indicators = EMPTY_INDICATORS,
  displayRangeStart,
  onSelectAnnotation,
  selectedAnnotationId,
  horizontalLineDrawingEnabled = false,
  pendingHorizontalLine,
  onPlaceHorizontalLine,
  currency,
  priceScaleTopMargin = 0.05,
  intraday = false,
  highlightedTimestamp,
  className,
}: CandlestickChartProps) {
  const containerRef = useRef<HTMLDivElement>(null)
  const chartRef = useRef<IChartApi | null>(null)
  const candlestickSeriesRef = useRef<ISeriesApi<SeriesType> | null>(null)
  const tradeMarkersRef = useRef<ISeriesMarkersPluginApi<Time> | null>(null)
  const annotationPriceSeriesRef = useRef<ISeriesApi<'Line'> | null>(null)
  const volumeSeriesRef = useRef<ISeriesApi<SeriesType> | null>(null)
  const indicatorSeriesRef = useRef<Map<string, ISeriesApi<'Line'>[]>>(
    new Map(),
  )
  const priceLineRef = useRef<IPriceLine | null>(null)
  const humanPriceLinesRef = useRef<Map<string, IPriceLine>>(new Map())
  const pendingPriceLineRef = useRef<IPriceLine | null>(null)
  const isInitialDataRef = useRef(true)
  const previousDisplayRangeRef = useRef<string | undefined>(undefined)
  const hasAnnotationBand =
    annotations?.some((annotation) => !isHumanHorizontalLine(annotation)) ??
    false

  // チャートの初期化 (マウント時のみ)
  useEffect(() => {
    const container = containerRef.current
    if (!container) return

    const isDark = document.documentElement.classList.contains('dark')
    const colors = getChartThemeColors(isDark)

    const chart = createChart(container, {
      ...getChartThemeOptions(colors),
      width: container.clientWidth,
      height: container.clientHeight,
    })
    chartRef.current = chart

    // ローソク足シリーズ
    const candlestickSeries = chart.addSeries(CandlestickSeries, {
      upColor: '#26a69a',
      downColor: '#ef5350',
      wickUpColor: '#26a69a',
      wickDownColor: '#ef5350',
      borderVisible: false,
      priceScaleId: 'right',
    })
    candlestickSeriesRef.current = candlestickSeries
    tradeMarkersRef.current = createSeriesMarkers(candlestickSeries)

    const annotationPriceSeries = chart.addSeries(LineSeries, {
      color: '#ef4444',
      lineWidth: 2,
      lineStyle: LineStyle.Dashed,
      priceScaleId: 'right',
      priceLineVisible: false,
      lastValueVisible: false,
      crosshairMarkerVisible: false,
      pointMarkersVisible: false,
      autoscaleInfoProvider: () => null,
    })
    annotationPriceSeriesRef.current = annotationPriceSeries

    // 出来高ヒストグラム
    const volumeSeries = chart.addSeries(HistogramSeries, {
      priceFormat: { type: 'volume' },
      priceScaleId: 'volume',
    })
    volumeSeriesRef.current = volumeSeries

    isInitialDataRef.current = true

    // コンテナサイズ追従
    const disconnectResizeObserver = observeChartResize(
      container,
      (width, height) => {
        chart.applyOptions({ width, height })
      },
    )

    // ダークモード追従: html 要素の class 変更を監視
    const disconnectThemeObserver = observeChartTheme(() => {
      const dark = document.documentElement.classList.contains('dark')
      const nextColors = getChartThemeColors(dark)
      chart.applyOptions(getChartThemeOptions(nextColors))
    })

    return () => {
      disconnectThemeObserver()
      disconnectResizeObserver()
      tradeMarkersRef.current?.detach()
      tradeMarkersRef.current = null
      if (candlestickSeriesRef.current != null) {
        for (const line of humanPriceLinesRef.current.values()) {
          candlestickSeriesRef.current.removePriceLine(line)
        }
        humanPriceLinesRef.current.clear()
        if (pendingPriceLineRef.current != null) {
          candlestickSeriesRef.current.removePriceLine(
            pendingPriceLineRef.current,
          )
          pendingPriceLineRef.current = null
        }
      }
      chart.remove()
      chartRef.current = null
      candlestickSeriesRef.current = null
      annotationPriceSeriesRef.current = null
      volumeSeriesRef.current = null
      indicatorSeriesRef.current.clear()
      priceLineRef.current = null
    }
  }, [])

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
          (item) =>
            Math.floor(Date.parse(item.timestamp) / 1000) === param.time,
        )
        const price = activeSeries.coordinateToPrice(point.y)
        if (bar == null || price == null || !Number.isFinite(price)) return

        onPlaceHorizontalLine({ timestamp: bar.timestamp, price })
        return
      }

      if (onSelectAnnotation == null) return

      const nearestLine = (annotations ?? [])
        .filter(isHumanHorizontalLine)
        .map((annotation) => ({
          id: annotation.id,
          distance: Math.abs(
            (activeSeries.priceToCoordinate(annotation.price) ??
              Number.POSITIVE_INFINITY) - point.y,
          ),
        }))
        .reduce<{ id: string; distance: number } | null>(
          (nearest, candidate) =>
            nearest == null || candidate.distance < nearest.distance
              ? candidate
              : nearest,
          null,
        )

      if (nearestLine != null && nearestLine.distance <= 8) {
        onSelectAnnotation(nearestLine.id)
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

  useEffect(() => {
    const series = candlestickSeriesRef.current
    if (series == null) return

    series.priceScale().applyOptions({
      scaleMargins: { top: priceScaleTopMargin, bottom: 0.25 },
    })
  }, [priceScaleTopMargin])

  useEffect(() => {
    chartRef.current?.applyOptions({
      timeScale: { timeVisible: intraday, secondsVisible: false },
    })
  }, [intraday])

  useEffect(() => {
    const series = volumeSeriesRef.current
    if (series == null) return

    series.priceScale().applyOptions({
      scaleMargins: { top: 0.8, bottom: hasAnnotationBand ? 0.06 : 0 },
    })
  }, [hasAnnotationBand])

  useEffect(() => {
    const series = candlestickSeriesRef.current
    if (series == null) return

    if (currency == null) {
      series.applyOptions({
        priceFormat: { type: 'price', precision: 2, minMove: 0.01 },
      })
      return
    }

    const formatter = new Intl.NumberFormat('ja-JP', {
      style: 'currency',
      currency,
      maximumFractionDigits: 2,
    })
    series.applyOptions({
      priceFormat: {
        type: 'custom',
        minMove: 0.01,
        formatter: (price: number) => formatter.format(price),
      },
    })
  }, [currency])

  // データ更新 (bars 変更時にシリーズのデータのみ差し替え)
  useEffect(() => {
    if (!candlestickSeriesRef.current || !volumeSeriesRef.current) return
    if (bars.length === 0) return

    const timeScale = chartRef.current?.timeScale()
    const visibleRange = timeScale?.getVisibleRange()
    candlestickSeriesRef.current.setData(toCandlestickData(bars))
    volumeSeriesRef.current.setData(toVolumeData(bars))

    const shouldResetVisibleRange =
      isInitialDataRef.current ||
      previousDisplayRangeRef.current !== displayRangeStart

    if (shouldResetVisibleRange) {
      const firstVisibleIndex =
        displayRangeStart == null
          ? -1
          : bars.findIndex(
              (bar) =>
                Date.parse(bar.timestamp) >= Date.parse(displayRangeStart),
            )
      if (firstVisibleIndex > 0 && timeScale != null) {
        timeScale.setVisibleLogicalRange({
          from: firstVisibleIndex - 0.5,
          to: bars.length - 0.5,
        })
      } else {
        timeScale?.fitContent()
      }
      isInitialDataRef.current = false
    } else if (visibleRange != null) {
      timeScale?.setVisibleRange(visibleRange)
    }

    previousDisplayRangeRef.current = displayRangeStart
  }, [bars, displayRangeStart])

  useEffect(() => {
    const chart = chartRef.current
    if (chart == null) return

    for (const series of indicatorSeriesRef.current.values()) {
      for (const lineSeries of series) chart.removeSeries(lineSeries)
    }
    indicatorSeriesRef.current.clear()

    const indicatorBars = toCandlestickData(bars).map(({ time, close }) => ({
      time,
      close,
    }))

    for (const indicator of indicators) {
      const lines = calculateChartIndicatorLines(indicator, indicatorBars)
      const series = lines.map(({ variant, points }) => {
        const lineSeries = chart.addSeries(LineSeries, {
          color: indicator.color,
          lineWidth: variant === 'band' ? 1 : 2,
          lineStyle: variant === 'band' ? LineStyle.Dashed : LineStyle.Solid,
          priceScaleId: 'right',
          priceLineVisible: false,
          lastValueVisible: false,
          crosshairMarkerVisible: false,
          pointMarkersVisible: false,
        })
        lineSeries.setData(points)
        return lineSeries
      })
      indicatorSeriesRef.current.set(indicator.id, series)
    }
  }, [bars, indicators])

  useEffect(() => {
    tradeMarkersRef.current?.setMarkers(
      toTradeMarkers(trades ?? [], bars, selectedTradeId),
    )
  }, [bars, trades, selectedTradeId])

  const { bandLayout, bandMarkers, bandPeriods, dateMarkerLayout } =
    useAnnotationOverlays({
      chartRef,
      candlestickSeriesRef,
      annotationPriceSeriesRef,
      priceLineRef,
      humanPriceLinesRef,
      pendingPriceLineRef,
      bars,
      annotations,
      selectedAnnotationId,
      pendingHorizontalLine,
      highlightedTimestamp,
    })

  return (
    <div className={cn('relative min-w-0', className)}>
      <div
        ref={containerRef}
        className={cn(
          'absolute inset-0',
          horizontalLineDrawingEnabled && 'cursor-crosshair',
        )}
      />
      {dateMarkerLayout != null && (
        <span
          aria-label={`${dateMarkerLayout.date} のチャート上の位置`}
          className="pointer-events-none absolute top-0 z-10 border-l border-dashed border-amber-400"
          role="img"
          style={{ left: dateMarkerLayout.x, bottom: dateMarkerLayout.bottom }}
        />
      )}
      {bandLayout != null && (
        <AnnotationChartBand
          markers={bandMarkers}
          periods={bandPeriods}
          width={bandLayout.width}
          bottom={bandLayout.bottom}
          selectedAnnotationId={selectedAnnotationId ?? null}
          onSelectAnnotation={onSelectAnnotation}
        />
      )}
      <ChartIndicatorLegend indicators={indicators} />
    </div>
  )
}
