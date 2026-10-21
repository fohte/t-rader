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
  type SeriesType,
  type Time,
} from 'lightweight-charts'
import { type CSSProperties, useEffect, useRef, useState } from 'react'

import { AnnotationChartBand } from '#components/annotations/annotation-chart-band'
import { useAnnotationOverlays } from '#components/candlestick-chart/use-annotation-overlays'
import { useChartIndicators } from '#components/candlestick-chart/use-chart-indicators'
import { useChartInteractions } from '#components/candlestick-chart/use-chart-interactions'
import { ChartIndicatorLegend } from '#components/chart-indicator-legend'
import {
  type ChartAnnotation,
  type HorizontalLinePoint,
  isAnnotationBandAnnotation,
  type TrendLinePoint,
} from '#lib/annotation-chart-utils'
import type { components } from '#lib/api/schema.gen'
import {
  CHART_INDICATOR_PANE_HEIGHT,
  type ChartIndicatorConfig,
} from '#lib/chart-indicator-settings'
import {
  getAnnotationSelectionColor,
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
  trendLineDrawingEnabled?: boolean
  pendingTrendLineStart?: TrendLinePoint | null
  pendingTrendLinePreviewPoint?: TrendLinePoint | null
  onPlaceTrendLinePoint?: (point: TrendLinePoint) => void
  currency?: ChartCurrency
  priceScaleTopMargin?: number
  intraday?: boolean
  highlightedTimestamp?: string
  className?: string
  style?: CSSProperties
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
  trendLineDrawingEnabled = false,
  pendingTrendLineStart,
  pendingTrendLinePreviewPoint,
  onPlaceTrendLinePoint,
  currency,
  priceScaleTopMargin = 0.05,
  intraday = false,
  highlightedTimestamp,
  className,
  style,
}: CandlestickChartProps) {
  const containerRef = useRef<HTMLDivElement>(null)
  const chartRef = useRef<IChartApi | null>(null)
  const [paneTops, setPaneTops] = useState<number[]>([])
  const updatePaneLayoutRef = useRef<() => void>(() => {})
  const lowerPaneCountRef = useRef(0)
  const candlestickSeriesRef = useRef<ISeriesApi<SeriesType> | null>(null)
  const tradeMarkersRef = useRef<ISeriesMarkersPluginApi<Time> | null>(null)
  const annotationPriceSeriesRef = useRef<ISeriesApi<'Line'> | null>(null)
  const trendLinePreviewSeriesRef = useRef<ISeriesApi<'Line'> | null>(null)
  const volumeSeriesRef = useRef<ISeriesApi<SeriesType> | null>(null)
  const indicatorSeriesRef = useRef<Map<string, ISeriesApi<SeriesType>[]>>(
    new Map(),
  )
  const priceLineRef = useRef<IPriceLine | null>(null)
  const isInitialDataRef = useRef(true)
  const previousDisplayRangeRef = useRef<string | undefined>(undefined)
  const hasAnnotationBand =
    annotations?.some(isAnnotationBandAnnotation) ?? false

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
    chart.applyOptions({ layout: { panes: { enableResize: false } } })
    let paneTopsAnimationFrame: number | null = null

    const updatePaneTops = () => {
      let top = 0
      const nextPaneTops = chart.panes().map((pane) => {
        const paneTop = top
        top += pane.getHeight()
        return paneTop
      })
      setPaneTops((currentPaneTops) =>
        currentPaneTops.length === nextPaneTops.length &&
        currentPaneTops.every(
          (currentTop, index) => currentTop === nextPaneTops[index],
        )
          ? currentPaneTops
          : nextPaneTops,
      )
    }
    const schedulePaneTopsUpdate = () => {
      if (paneTopsAnimationFrame != null) {
        cancelAnimationFrame(paneTopsAnimationFrame)
      }
      paneTopsAnimationFrame = requestAnimationFrame(() => {
        paneTopsAnimationFrame = null
        updatePaneTops()
      })
    }
    const updatePaneLayout = (chartHeight = container.clientHeight) => {
      const lowerPaneCount = lowerPaneCountRef.current
      const mainPaneHeight = Math.max(
        1,
        chartHeight -
          chart.timeScale().height() -
          lowerPaneCount * CHART_INDICATOR_PANE_HEIGHT,
      )
      const panes = chart.panes()
      panes[0]?.setStretchFactor(mainPaneHeight)
      for (let paneIndex = 1; paneIndex <= lowerPaneCount; paneIndex += 1) {
        panes[paneIndex]?.setStretchFactor(CHART_INDICATOR_PANE_HEIGHT)
      }
      schedulePaneTopsUpdate()
    }
    updatePaneLayoutRef.current = updatePaneLayout

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
      color: getAnnotationSelectionColor(isDark),
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

    trendLinePreviewSeriesRef.current = chart.addSeries(LineSeries, {
      color: '#facc15',
      lineWidth: 2,
      lineStyle: LineStyle.Dashed,
      priceScaleId: 'right',
      priceLineVisible: false,
      lastValueVisible: false,
      crosshairMarkerVisible: false,
      pointMarkersVisible: false,
      autoscaleInfoProvider: () => null,
    })

    isInitialDataRef.current = true

    // コンテナサイズ追従
    const disconnectResizeObserver = observeChartResize(
      container,
      (width, height) => {
        chart.applyOptions({ width, height })
        updatePaneLayout(height)
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
      if (paneTopsAnimationFrame != null) {
        cancelAnimationFrame(paneTopsAnimationFrame)
      }
      tradeMarkersRef.current?.detach()
      tradeMarkersRef.current = null
      chart.remove()
      chartRef.current = null
      candlestickSeriesRef.current = null
      annotationPriceSeriesRef.current = null
      trendLinePreviewSeriesRef.current = null
      volumeSeriesRef.current = null
      indicatorSeriesRef.current.clear()
      priceLineRef.current = null
      updatePaneLayoutRef.current = () => {}
    }
  }, [])

  const { trendLineCursorPoint } = useChartInteractions({
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
  })

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

  useChartIndicators({
    chartRef,
    indicatorSeriesRef,
    lowerPaneCountRef,
    updatePaneLayoutRef,
    bars,
    indicators,
  })

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
      trendLinePreviewSeriesRef,
      priceLineRef,
      bars,
      annotations,
      selectedAnnotationId,
      pendingHorizontalLine,
      trendLineDrawingEnabled,
      pendingTrendLineStart,
      pendingTrendLinePreviewPoint:
        pendingTrendLinePreviewPoint ?? trendLineCursorPoint,
      highlightedTimestamp,
    })
  const dateMarkerStyle:
    | (CSSProperties & {
        '--chart-date-marker-left': string
        '--chart-date-marker-bottom': string
      })
    | undefined =
    dateMarkerLayout == null
      ? undefined
      : {
          '--chart-date-marker-left': `${dateMarkerLayout.x.toString()}px`,
          '--chart-date-marker-bottom': `${dateMarkerLayout.bottom.toString()}px`,
        }

  return (
    <div className={cn('relative min-w-0', className)} style={style}>
      <div
        ref={containerRef}
        className={cn(
          'absolute inset-0',
          (horizontalLineDrawingEnabled || trendLineDrawingEnabled) &&
            'cursor-crosshair',
        )}
      />
      {dateMarkerLayout != null && (
        <span
          aria-label={`${dateMarkerLayout.date} のチャート上の位置`}
          className="pointer-events-none absolute top-0 z-10 border-l border-dashed border-chart-3 [left:var(--chart-date-marker-left)] [bottom:var(--chart-date-marker-bottom)]"
          role="img"
          style={dateMarkerStyle}
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
      <ChartIndicatorLegend indicators={indicators} paneTops={paneTops} />
    </div>
  )
}
