import {
  CandlestickSeries,
  ColorType,
  createChart,
  HistogramSeries,
  type IChartApi,
  type ISeriesApi,
  type SeriesType,
} from 'lightweight-charts'
import { useEffect, useRef } from 'react'

import type { components } from '@/lib/api/schema.gen'
import { toCandlestickData, toVolumeData } from '@/lib/chart-utils'

type Bar = components['schemas']['Bar']

interface CandlestickChartProps {
  bars: Bar[]
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

export function CandlestickChart({ bars, className }: CandlestickChartProps) {
  const containerRef = useRef<HTMLDivElement>(null)
  const chartRef = useRef<IChartApi | null>(null)
  const candlestickSeriesRef = useRef<ISeriesApi<SeriesType> | null>(null)
  const volumeSeriesRef = useRef<ISeriesApi<SeriesType> | null>(null)
  const isInitialDataRef = useRef(true)

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

  return <div ref={containerRef} className={className} />
}
