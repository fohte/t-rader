import {
  CandlestickSeries,
  ColorType,
  createChart,
  HistogramSeries,
  type IChartApi,
} from 'lightweight-charts'
import { useEffect, useRef } from 'react'

import type { components } from '@/lib/api/schema.gen'
import { toCandlestickData, toVolumeData } from '@/lib/chart-utils'

type Bar = components['schemas']['Bar']

interface CandlestickChartProps {
  bars: Bar[]
  className?: string
}

export function CandlestickChart({ bars, className }: CandlestickChartProps) {
  const containerRef = useRef<HTMLDivElement>(null)
  const chartRef = useRef<IChartApi | null>(null)

  useEffect(() => {
    const container = containerRef.current
    if (!container) return

    const isDark = document.documentElement.classList.contains('dark')

    const chart = createChart(container, {
      layout: {
        background: {
          type: ColorType.Solid,
          color: isDark ? '#1a1a1a' : '#ffffff',
        },
        textColor: isDark ? '#d1d5db' : '#374151',
      },
      grid: {
        vertLines: { color: isDark ? '#2d2d2d' : '#e5e7eb' },
        horzLines: { color: isDark ? '#2d2d2d' : '#e5e7eb' },
      },
      width: container.clientWidth,
      height: container.clientHeight,
      timeScale: {
        borderColor: isDark ? '#3f3f46' : '#d1d5db',
      },
      rightPriceScale: {
        borderColor: isDark ? '#3f3f46' : '#d1d5db',
      },
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
    candlestickSeries.setData(toCandlestickData(bars))

    // 出来高ヒストグラム
    const volumeSeries = chart.addSeries(HistogramSeries, {
      priceFormat: { type: 'volume' },
      priceScaleId: 'volume',
    })
    volumeSeries.priceScale().applyOptions({
      scaleMargins: { top: 0.8, bottom: 0 },
    })
    volumeSeries.setData(toVolumeData(bars))

    chart.timeScale().fitContent()

    // コンテナサイズ追従
    const resizeObserver = new ResizeObserver((entries) => {
      for (const entry of entries) {
        const { width, height } = entry.contentRect
        chart.applyOptions({ width, height })
      }
    })
    resizeObserver.observe(container)

    return () => {
      resizeObserver.disconnect()
      chart.remove()
      chartRef.current = null
    }
  }, [bars])

  return <div ref={containerRef} className={className} />
}
