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
import { ChartIndicatorLegend } from '#components/chart-indicator-legend'
import type { ChartAnnotation } from '#lib/annotation-chart-utils'
import type { components } from '#lib/api/schema.gen'
import { calculateChartIndicatorLines } from '#lib/chart-indicator-calculations'
import {
  CHART_INDICATOR_PANE_HEIGHT,
  type ChartIndicatorConfig,
} from '#lib/chart-indicator-settings'
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
  const updatePaneTopsRef = useRef<() => void>(() => {})
  const candlestickSeriesRef = useRef<ISeriesApi<SeriesType> | null>(null)
  const tradeMarkersRef = useRef<ISeriesMarkersPluginApi<Time> | null>(null)
  const annotationPriceSeriesRef = useRef<ISeriesApi<'Line'> | null>(null)
  const volumeSeriesRef = useRef<ISeriesApi<SeriesType> | null>(null)
  const indicatorSeriesRef = useRef<Map<string, ISeriesApi<SeriesType>[]>>(
    new Map(),
  )
  const priceLineRef = useRef<IPriceLine | null>(null)
  const isInitialDataRef = useRef(true)
  const previousDisplayRangeRef = useRef<string | undefined>(undefined)
  const hasAnnotations = (annotations?.length ?? 0) > 0

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
    updatePaneTopsRef.current = updatePaneTops

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
    updatePaneTops()

    isInitialDataRef.current = true

    // コンテナサイズ追従
    const disconnectResizeObserver = observeChartResize(
      container,
      (width, height) => {
        chart.applyOptions({ width, height })
        updatePaneTops()
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
      chart.remove()
      chartRef.current = null
      candlestickSeriesRef.current = null
      annotationPriceSeriesRef.current = null
      volumeSeriesRef.current = null
      indicatorSeriesRef.current.clear()
      priceLineRef.current = null
      updatePaneTopsRef.current = () => {}
    }
  }, [])

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
      scaleMargins: { top: 0.8, bottom: hasAnnotations ? 0.06 : 0 },
    })
  }, [hasAnnotations])

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
      for (const indicatorSeries of series) {
        chart.removeSeries(indicatorSeries)
      }
    }
    indicatorSeriesRef.current.clear()
    while (chart.panes().length > 1) {
      chart.removePane(chart.panes().length - 1)
    }

    const indicatorBars = toCandlestickData(bars).map(({ time, close }) => ({
      time,
      close,
    }))
    let lowerPaneIndex = 0

    for (const indicator of indicators) {
      const lines = calculateChartIndicatorLines(indicator, indicatorBars)
      const isLowerPane = indicator.kind === 'rsi' || indicator.kind === 'macd'
      const paneIndex = isLowerPane ? lowerPaneIndex + 1 : 0
      if (isLowerPane) lowerPaneIndex += 1

      const series: ISeriesApi<SeriesType>[] = []
      for (const { variant, points } of lines) {
        if (variant === 'histogram') {
          const histogramSeries = chart.addSeries(
            HistogramSeries,
            {
              priceScaleId: 'right',
              priceLineVisible: false,
              lastValueVisible: false,
            },
            paneIndex,
          )
          histogramSeries.setData(
            points.map((point) => ({
              ...point,
              color:
                point.value >= 0
                  ? 'rgba(38, 166, 154, 0.7)'
                  : 'rgba(239, 83, 80, 0.7)',
            })),
          )
          series.push(histogramSeries)
          continue
        }

        const lineSeries = chart.addSeries(
          LineSeries,
          {
            color: variant === 'signal' ? '#a1a1aa' : indicator.color,
            lineWidth: variant === 'band' || variant === 'signal' ? 1 : 2,
            lineStyle:
              variant === 'band' || variant === 'signal'
                ? LineStyle.Dashed
                : LineStyle.Solid,
            priceScaleId: 'right',
            priceLineVisible: false,
            lastValueVisible: false,
            crosshairMarkerVisible: false,
            pointMarkersVisible: false,
            ...(indicator.kind === 'rsi'
              ? {
                  autoscaleInfoProvider: () => ({
                    priceRange: { minValue: 0, maxValue: 100 },
                  }),
                }
              : {}),
          },
          paneIndex,
        )
        lineSeries.setData(points)

        if (indicator.kind === 'rsi') {
          for (const price of [30, 70]) {
            lineSeries.createPriceLine({
              price,
              color: '#71717a',
              lineWidth: 1,
              lineStyle: LineStyle.Dashed,
              axisLabelVisible: true,
              title: price.toString(),
            })
          }
        } else if (indicator.kind === 'macd' && variant === 'main') {
          lineSeries.createPriceLine({
            price: 0,
            color: '#71717a',
            lineWidth: 1,
            lineStyle: LineStyle.Dashed,
            axisLabelVisible: false,
            title: '',
          })
        }

        series.push(lineSeries)
      }
      indicatorSeriesRef.current.set(indicator.id, series)
    }

    for (let paneIndex = 1; paneIndex <= lowerPaneIndex; paneIndex += 1) {
      chart.panes()[paneIndex]?.setHeight(CHART_INDICATOR_PANE_HEIGHT)
    }
    updatePaneTopsRef.current()
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
      bars,
      annotations,
      selectedAnnotationId,
      highlightedTimestamp,
    })

  return (
    <div className={cn('relative min-w-0', className)} style={style}>
      <div ref={containerRef} className="absolute inset-0" />
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
      <ChartIndicatorLegend indicators={indicators} paneTops={paneTops} />
    </div>
  )
}
