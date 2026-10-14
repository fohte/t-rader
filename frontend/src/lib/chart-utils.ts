import type {
  CandlestickData,
  HistogramData,
  SeriesMarker,
  UTCTimestamp,
} from 'lightweight-charts'

import type { components } from '#lib/api/schema.gen'

type Bar = components['schemas']['Bar']
type Trade = components['schemas']['TradeListItem']
export type ChartCurrency = 'JPY' | 'USD'

const chartDateFormatter = new Intl.DateTimeFormat('en-US', {
  timeZone: 'Asia/Tokyo',
  year: 'numeric',
  month: '2-digit',
  day: '2-digit',
})

/** 日足とアノテーションの日付を揃えるため、日本時間で取引日を返す */
export function getChartBarDate(value: string): string | null {
  const date = new Date(value)
  if (!Number.isFinite(date.getTime())) return null

  const parts = chartDateFormatter.formatToParts(date)
  const year = parts.find((part) => part.type === 'year')?.value
  const month = parts.find((part) => part.type === 'month')?.value
  const day = parts.find((part) => part.type === 'day')?.value
  if (year == null || month == null || day == null) return null

  return `${year.padStart(4, '0')}-${month}-${day}`
}

/** 市場コードから価格表示に使う通貨を返す */
export function getChartCurrency(
  market: string | null | undefined,
): ChartCurrency | undefined {
  if (market === 'TSE') return 'JPY'
  if (market === 'US') return 'USD'
  return undefined
}

/** ISO 8601 タイムスタンプを Unix タイムスタンプ (秒) に変換する */
function toUTCTimestamp(isoTimestamp: string): UTCTimestamp {
  // eslint-disable-next-line @typescript-eslint/no-unsafe-type-assertion -- UTCTimestamp はブランド型で as が唯一の変換手段
  return Math.floor(new Date(isoTimestamp).getTime() / 1000) as UTCTimestamp
}

/** API レスポンスの Bar をローソク足データに変換する */
export function toCandlestickData(bars: Bar[]): CandlestickData[] {
  return bars.map((bar) => ({
    time: toUTCTimestamp(bar.timestamp),
    open: bar.open,
    high: bar.high,
    low: bar.low,
    close: bar.close,
  }))
}

/** API レスポンスの Bar を出来高ヒストグラムデータに変換する */
export function toVolumeData(bars: Bar[]): HistogramData[] {
  return bars.map((bar) => ({
    time: toUTCTimestamp(bar.timestamp),
    value: bar.volume,
    // 陽線は緑系、陰線は赤系
    color:
      bar.close >= bar.open
        ? 'rgba(38, 166, 154, 0.5)'
        : 'rgba(239, 83, 80, 0.5)',
  }))
}

/** API の約定を同日の最初の足に対応するチャートマーカーへ変換する */
export function toTradeMarkers(
  trades: Trade[],
  bars: Bar[],
): SeriesMarker<UTCTimestamp>[] {
  const firstBarByDate = new Map<string, UTCTimestamp>()

  for (const bar of bars) {
    const milliseconds = Date.parse(bar.timestamp)
    if (!Number.isFinite(milliseconds)) continue

    const date = new Date(milliseconds).toISOString().slice(0, 10)
    if (firstBarByDate.has(date)) continue

    firstBarByDate.set(date, toUTCTimestamp(bar.timestamp))
  }

  return trades
    .flatMap((trade): SeriesMarker<UTCTimestamp>[] => {
      const time = firstBarByDate.get(trade.date)
      if (time == null) return []

      const isBuy = trade.side === 'buy'
      return [
        {
          id: trade.id,
          time,
          position: isBuy ? 'belowBar' : 'aboveBar',
          color: isBuy ? '#26a69a' : '#ef5350',
          shape: isBuy ? 'arrowUp' : 'arrowDown',
        },
      ]
    })
    .sort((left, right) => left.time - right.time)
}
