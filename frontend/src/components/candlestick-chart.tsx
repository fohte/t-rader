import {
  CandlestickSeries,
  ColorType,
  createChart,
  HistogramSeries,
  type IChartApi,
  type IPriceLine,
  type ISeriesApi,
  LineStyle,
  type SeriesType,
} from 'lightweight-charts'
import { useEffect, useRef, useState } from 'react'

import { AnnotationChartBand } from '#components/annotations/annotation-chart-band'
import {
  bucketAnnotationTimestamp,
  type ChartAnnotation,
} from '#lib/annotation-chart-utils'
import type { components } from '#lib/api/schema.gen'
import {
  type ChartCurrency,
  toCandlestickData,
  toVolumeData,
} from '#lib/chart-utils'
import { cn } from '#lib/utils'

type Bar = components['schemas']['Bar']

interface BandLayout {
  width: number
  bottom: number
}

interface CandlestickChartProps {
  bars: Bar[]
  annotations?: ChartAnnotation[]
  onSelectAnnotation?: (id: string) => void
  selectedAnnotationId?: string | null
  currency?: ChartCurrency
  intraday?: boolean
  className?: string
}

function getThemeColors(isDark: boolean) {
  return {
    background: isDark ? '#1a1a1a' : '#ffffff',
    textColor: isDark ? '#d1d5db' : '#374151',
    gridColor: isDark ? '#2d2d2d' : '#e5e7eb',
    borderColor: isDark ? '#3f3f46' : '#d1d5db',
  }
}

export function CandlestickChart({
  bars,
  annotations,
  onSelectAnnotation,
  selectedAnnotationId,
  currency,
  intraday = false,
  className,
}: CandlestickChartProps) {
  const containerRef = useRef<HTMLDivElement>(null)
  const chartRef = useRef<IChartApi | null>(null)
  const candlestickSeriesRef = useRef<ISeriesApi<SeriesType> | null>(null)
  const volumeSeriesRef = useRef<ISeriesApi<SeriesType> | null>(null)
  const priceLineRef = useRef<IPriceLine | null>(null)
  const isInitialDataRef = useRef(true)
  const [bandLayout, setBandLayout] = useState<BandLayout | null>(null)
  const [bandMarkers, setBandMarkers] = useState<{ id: string; x: number }[]>(
    [],
  )

  // チャートの初期化 (マウント時のみ)
  useEffect(() => {
    const container = containerRef.current
    if (!container) return

    const isDark = document.documentElement.classList.contains('dark')
    const colors = getThemeColors(isDark)

    const chart = createChart(container, {
      layout: {
        background: { type: ColorType.Solid, color: colors.background },
        textColor: colors.textColor,
      },
      grid: {
        vertLines: { color: colors.gridColor },
        horzLines: { color: colors.gridColor },
      },
      width: container.clientWidth,
      height: container.clientHeight,
      timeScale: { borderColor: colors.borderColor },
      rightPriceScale: { borderColor: colors.borderColor },
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
    candlestickSeries.priceScale().applyOptions({
      scaleMargins: { top: 0.05, bottom: 0.25 },
    })
    candlestickSeriesRef.current = candlestickSeries

    // 出来高ヒストグラム
    const volumeSeries = chart.addSeries(HistogramSeries, {
      priceFormat: { type: 'volume' },
      priceScaleId: 'volume',
    })
    // チャート下端のアノテーション帯と出来高を重ねない。
    volumeSeries.priceScale().applyOptions({
      scaleMargins: { top: 0.8, bottom: 0.06 },
    })
    volumeSeriesRef.current = volumeSeries

    isInitialDataRef.current = true

    // コンテナサイズ追従
    const resizeObserver = new ResizeObserver((entries) => {
      for (const entry of entries) {
        const { width, height } = entry.contentRect
        chart.applyOptions({ width, height })
      }
    })
    resizeObserver.observe(container)

    // ダークモード追従: html 要素の class 変更を監視
    const mutationObserver = new MutationObserver(() => {
      const dark = document.documentElement.classList.contains('dark')
      const c = getThemeColors(dark)
      chart.applyOptions({
        layout: {
          background: { type: ColorType.Solid, color: c.background },
          textColor: c.textColor,
        },
        grid: {
          vertLines: { color: c.gridColor },
          horzLines: { color: c.gridColor },
        },
        timeScale: { borderColor: c.borderColor },
        rightPriceScale: { borderColor: c.borderColor },
      })
    })
    mutationObserver.observe(document.documentElement, {
      attributes: true,
      attributeFilter: ['class'],
    })

    return () => {
      mutationObserver.disconnect()
      resizeObserver.disconnect()
      chart.remove()
      chartRef.current = null
      candlestickSeriesRef.current = null
      volumeSeriesRef.current = null
      priceLineRef.current = null
    }
  }, [])

  useEffect(() => {
    chartRef.current?.applyOptions({
      timeScale: { timeVisible: intraday, secondsVisible: false },
    })
  }, [intraday])

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
    const chart = chartRef.current
    if (chart == null) return

    const timeScale = chart.timeScale()
    const bucketedAnnotations = (annotations ?? []).flatMap((annotation) => {
      const time = bucketAnnotationTimestamp(annotation.timestamp, bars)
      return time == null ? [] : [{ id: annotation.id, time }]
    })
    const updateBand = () => {
      const width = timeScale.width()
      setBandLayout({ width, bottom: timeScale.height() })
      setBandMarkers(
        bucketedAnnotations.flatMap((annotation) => {
          const x = timeScale.timeToCoordinate(annotation.time)
          return x == null || x < 0 || x > width
            ? []
            : [{ id: annotation.id, x }]
        }),
      )
    }

    updateBand()
    timeScale.subscribeVisibleTimeRangeChange(updateBand)
    timeScale.subscribeSizeChange(updateBand)

    return () => {
      timeScale.unsubscribeVisibleTimeRangeChange(updateBand)
      timeScale.unsubscribeSizeChange(updateBand)
    }
  }, [annotations, bars])

  useEffect(() => {
    const series = candlestickSeriesRef.current
    if (series == null) return

    if (priceLineRef.current != null) {
      series.removePriceLine(priceLineRef.current)
      priceLineRef.current = null
    }

    const selectedAnnotation = annotations?.find(
      (annotation) => annotation.id === selectedAnnotationId,
    )
    if (selectedAnnotation?.price == null) return

    priceLineRef.current = series.createPriceLine({
      price: selectedAnnotation.price,
      color: '#ef4444',
      lineWidth: 2,
      lineStyle: LineStyle.Dashed,
      lineVisible: true,
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
  }, [annotations, selectedAnnotationId])

  return (
    <div className={cn('relative min-w-0', className)}>
      <div ref={containerRef} className="absolute inset-0" />
      {bandLayout != null && (
        <AnnotationChartBand
          markers={bandMarkers}
          width={bandLayout.width}
          bottom={bandLayout.bottom}
          selectedAnnotationId={selectedAnnotationId ?? null}
          onSelectAnnotation={onSelectAnnotation}
        />
      )}
    </div>
  )
}
