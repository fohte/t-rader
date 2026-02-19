import type { CandlestickData, HistogramData } from 'lightweight-charts'

import type { components } from '@/lib/api/schema.gen'

type Bar = components['schemas']['Bar']

/** API レスポンスの Bar をローソク足データに変換する */
export function toCandlestickData(bars: Bar[]): CandlestickData[] {
  return bars.map((bar) => ({
    time: bar.timestamp.slice(0, 10),
    open: Number(bar.open),
    high: Number(bar.high),
    low: Number(bar.low),
    close: Number(bar.close),
  }))
}

/** API レスポンスの Bar を出来高ヒストグラムデータに変換する */
export function toVolumeData(bars: Bar[]): HistogramData[] {
  return bars.map((bar) => {
    const open = Number(bar.open)
    const close = Number(bar.close)
    return {
      time: bar.timestamp.slice(0, 10),
      value: bar.volume,
      // 陽線は緑系、陰線は赤系
      color:
        close >= open ? 'rgba(38, 166, 154, 0.5)' : 'rgba(239, 83, 80, 0.5)',
    }
  })
}
