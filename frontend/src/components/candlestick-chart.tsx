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
import { useEffect, useRef } from 'react'

import { AnnotationChartBand } from '#components/annotations/annotation-chart-band'
import { useAnnotationOverlays } from '#components/candlestick-chart/use-annotation-overlays'
import type { ChartAnnotation } from '#lib/annotation-chart-utils'
import type { components } from '#lib/api/schema.gen'
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

interface CandlestickChartProps {
  bars: Bar[]
  trades?: TradeMarker[]
  selectedTradeId?: string
  annotations?: ChartAnnotation[]
  onSelectAnnotation?: (id: string) => void
  selectedAnnotationId?: string | null
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
  onSelectAnnotation,
  selectedAnnotationId,
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
  const priceLineRef = useRef<IPriceLine | null>(null)
  const isInitialDataRef = useRef(true)
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
      chart.remove()
      chartRef.current = null
      candlestickSeriesRef.current = null
      annotationPriceSeriesRef.current = null
      volumeSeriesRef.current = null
      priceLineRef.current = null
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

    candlestickSeriesRef.current.setData(toCandlestickData(bars))
    volumeSeriesRef.current.setData(toVolumeData(bars))

    // 初回データ設定時のみ fitContent でコンテンツ全体を表示
    if (isInitialDataRef.current) {
      chartRef.current?.timeScale().fitContent()
      isInitialDataRef.current = false
    }
  }, [bars])

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
    <div className={cn('relative min-w-0', className)}>
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
    </div>
  )
}
