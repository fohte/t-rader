import { formatYen } from '#components/trades/format'
import type { components } from '#lib/api/schema.gen'

type TradeDetail = components['schemas']['TradeDetailResponse']
type Trade = components['schemas']['TradeListItem']
type Bar = components['schemas']['Bar']

export type TradeDetailChartState =
  | { status: 'loading' }
  | { status: 'error' }
  | { status: 'empty' }
  | { status: 'ready'; bars: Bar[]; trades: Trade[] }

export function toTradeListItem(trade: TradeDetail): Trade {
  return {
    id: trade.id,
    strategy_id: trade.strategy_id,
    symbol: trade.symbol,
    side: trade.side,
    qty: trade.qty,
    price: trade.price,
    fee: trade.fee,
    date: trade.date,
    source: trade.source,
    note: trade.note,
    note_count: 0,
    created_at: trade.created_at,
    updated_at: trade.updated_at,
  }
}

export function getTradeDetailChartRange(tradeDate: string | undefined) {
  if (tradeDate == null) return null

  const center = new Date(`${tradeDate}T00:00:00.000Z`)
  const from = new Date(center)
  const to = new Date(center)
  from.setUTCDate(from.getUTCDate() - 14)
  to.setUTCDate(to.getUTCDate() + 14)

  return {
    from: from.toISOString().slice(0, 10),
    to: to.toISOString().slice(0, 10),
  }
}

export function createTradeDetailChartState({
  bars,
  barsPending,
  barsError,
  trades,
  selectedTrade,
}: {
  bars: Bar[] | undefined
  barsPending: boolean
  barsError: boolean
  trades: Trade[] | undefined
  selectedTrade: TradeDetail
}): TradeDetailChartState {
  if (barsPending) return { status: 'loading' }
  if (barsError) return { status: 'error' }
  if (bars == null || bars.length === 0) return { status: 'empty' }

  const tradeMarkers = trades ?? []
  const tradesWithSelected = tradeMarkers.some(
    (trade) => trade.id === selectedTrade.id,
  )
    ? tradeMarkers
    : [...tradeMarkers, toTradeListItem(selectedTrade)]

  return { status: 'ready', bars, trades: tradesWithSelected }
}

export function createTradeReflectionDraft(
  trade: TradeDetail,
  stockName: string | null,
): { title: string; body: string } {
  const isBuy = trade.side === 'buy'
  const sideLabel = isBuy ? '買' : '売'
  const actionLabel = isBuy ? '買った' : '売った'
  const lines = [
    trade.date +
      ' ' +
      sideLabel +
      ' ' +
      trade.qty.toLocaleString() +
      ' 株 @' +
      formatYen(trade.price),
  ]

  if (trade.side === 'sell' && trade.realized_pnl != null) {
    lines.push('実現損益 ' + formatYen(trade.realized_pnl, true))
  }

  lines.push('', '[[stock:' + trade.symbol + ']]')

  return {
    title: (stockName ?? trade.symbol) + 'を' + actionLabel + '振り返り',
    body: lines.join('\n'),
  }
}
