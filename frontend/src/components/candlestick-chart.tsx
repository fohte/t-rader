import {
  CandlestickSeries,
  ColorType,
  createChart,
  createSeriesMarkers,
  HistogramSeries,
  type IChartApi,
  type ISeriesApi,
  type ISeriesMarkersPluginApi,
  type SeriesMarker,
  type SeriesType,
  type Time,
  type UTCTimestamp,
} from 'lightweight-charts'
import { useEffect, useRef } from 'react'

import type { components } from '@/lib/api/schema.gen'
import { toCandlestickData, toVolumeData } from '@/lib/chart-utils'

type Bar = components['schemas']['Bar']

export interface ChartAnnotation {
  id: string
  /** ピン番号 (A1, A2, ...) */
  label: string
  /** ISO 8601 timestamp */
  timestamp: string
  target_kind: string
  status: string
  text: string
}

interface CandlestickChartProps {
  bars: Bar[]
  annotations?: ChartAnnotation[]
  onSelectAnnotation?: (id: string) => void
  selectedAnnotationId?: string | null
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
  className,
}: CandlestickChartProps) {
  const containerRef = useRef<HTMLDivElement>(null)
  const chartRef = useRef<IChartApi | null>(null)
  const candlestickSeriesRef = useRef<ISeriesApi<SeriesType> | null>(null)
  const volumeSeriesRef = useRef<ISeriesApi<SeriesType> | null>(null)
  const markersPluginRef = useRef<ISeriesMarkersPluginApi<Time> | null>(null)
  const isInitialDataRef = useRef(true)
  // クリックハンドラが annotations を見るたびに timestamp を parse すると毎クリック O(n) の Date 構築が走るので、
  // annotations 変更時に 1 度だけ秒に換算して保持する。
  const parsedAnnotationsRef = useRef<
    (ChartAnnotation & { timeSec: number })[]
  >([])
  const onSelectRef = useRef(onSelectAnnotation)
  useEffect(() => {
    parsedAnnotationsRef.current = (annotations ?? []).map((a) => ({
      ...a,
      timeSec: Math.floor(new Date(a.timestamp).getTime() / 1000),
    }))
  }, [annotations])
  useEffect(() => {
    onSelectRef.current = onSelectAnnotation
  }, [onSelectAnnotation])

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
    volumeSeries.priceScale().applyOptions({
      scaleMargins: { top: 0.8, bottom: 0 },
    })
    volumeSeriesRef.current = volumeSeries

    markersPluginRef.current = createSeriesMarkers(candlestickSeries)

    chart.subscribeClick((param) => {
      const handler = onSelectRef.current
      const list = parsedAnnotationsRef.current
      if (handler == null || list.length === 0 || param.time == null) {
        return
      }
      // Lightweight Charts の Time は UTC 秒の number または BusinessDay/string。
      // ここでは number 形式 (UTCTimestamp) のみ扱う。
      if (typeof param.time !== 'number') return
      const t = param.time
      let best: (ChartAnnotation & { timeSec: number }) | null = null
      let bestDiff = Infinity
      for (const a of list) {
        const diff = Math.abs(a.timeSec - t)
        if (diff < bestDiff) {
          bestDiff = diff
          best = a
        }
      }
      // 日足前提のヒューリスティクス: 隣接ローソク 1 本 (1 日) では誤検出が多く、
      // 週末や祝日を跨いだクリックも拾いたいので 3 日まで広げる。
      if (best != null && bestDiff <= 60 * 60 * 24 * 3) {
        handler(best.id)
      }
    })

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
      markersPluginRef.current = null
    }
  }, [])

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
    const plugin = markersPluginRef.current
    if (plugin == null) return
    const list = annotations ?? []
    const markers: SeriesMarker<Time>[] = list
      .map((a) => {
        // eslint-disable-next-line @typescript-eslint/no-unsafe-type-assertion -- UTCTimestamp はブランド型
        const time = Math.floor(
          new Date(a.timestamp).getTime() / 1000,
        ) as UTCTimestamp
        const isSelected = a.id === selectedAnnotationId
        const color = isSelected
          ? '#ef4444'
          : a.status === 'approved'
            ? '#71717a'
            : '#ef4444'
        return {
          id: a.id,
          time,
          position:
            a.target_kind === 'signal'
              ? ('belowBar' as const)
              : ('aboveBar' as const),
          color,
          shape:
            a.target_kind === 'signal'
              ? ('arrowUp' as const)
              : ('circle' as const),
          text: a.label,
        }
      })
      .sort((x, y) => (x.time as number) - (y.time as number))
    plugin.setMarkers(markers)
  }, [annotations, selectedAnnotationId])

  return <div ref={containerRef} className={className} />
}
