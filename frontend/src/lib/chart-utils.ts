import type {
  CandlestickData,
  HistogramData,
  UTCTimestamp,
} from 'lightweight-charts'

import type { components } from '@/lib/api/schema.gen'

type Bar = components['schemas']['Bar']

/** ISO 8601 タイムスタンプを Unix タイムスタンプ (秒) に変換する */
function toUTCTimestamp(isoTimestamp: string): UTCTimestamp {
  return Math.floor(new Date(isoTimestamp).getTime() / 1000) as UTCTimestamp
}

/** API レスポンスの Bar をローソク足データに変換する */
export function toCandlestickData(bars: Bar[]): CandlestickData[] {
  return bars.map((bar) => ({
    time: toUTCTimestamp(bar.timestamp),
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
      time: toUTCTimestamp(bar.timestamp),
      value: bar.volume,
      // 陽線は緑系、陰線は赤系
      color:
        close >= open ? 'rgba(38, 166, 154, 0.5)' : 'rgba(239, 83, 80, 0.5)',
    }
  })
}
