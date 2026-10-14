import type {
  CandlestickData,
  HistogramData,
  SeriesMarker,
  UTCTimestamp,
} from 'lightweight-charts'

import type { components } from '#lib/api/schema.gen'

type Bar = components['schemas']['Bar']
type Trade = components['schemas']['TradeListItem']
export type TradeMarker = Pick<Trade, 'date' | 'id' | 'side'>
export type ChartCurrency = 'JPY' | 'USD'
export const TRADE_MARKER_COLORS = {
  buy: '#26a69a',
  sell: '#ef5350',
} as const

/** 市場コードから価格表示に使う通貨を返す */
export function getChartCurrency(
  market: string | null | undefined,
): ChartCurrency | undefined {
  if (market === 'TSE') return 'JPY'
  if (market === 'US') return 'USD'
  return undefined
}

/** 銘柄 ID と市場から取引チャートの価格表示通貨を返す */
export function getChartCurrencyForStockId(
  stockId: string,
  market: string | null | undefined,
): ChartCurrency | undefined {
  if (stockId.startsWith('US:')) return 'USD'
  if (!stockId.includes(':')) return 'JPY'
  return getChartCurrency(market)
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
  trades: TradeMarker[],
  bars: Bar[],
  selectedTradeId?: string,
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
      const isSelected = trade.id === selectedTradeId
      return [
        {
          id: trade.id,
          time,
          position: isBuy ? 'belowBar' : 'aboveBar',
          color: isBuy ? TRADE_MARKER_COLORS.buy : TRADE_MARKER_COLORS.sell,
          shape: isBuy ? 'arrowUp' : 'arrowDown',
          ...(isSelected ? { text: '対象', size: 2 } : {}),
        },
      ]
    })
    .sort(
      (left, right) =>
        left.time - right.time ||
        Number(left.id === selectedTradeId) -
          Number(right.id === selectedTradeId),
    )
}
