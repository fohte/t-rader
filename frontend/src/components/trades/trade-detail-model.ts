import { formatYen } from '#components/trades/format'
import type { components } from '#lib/api/schema.gen'

type TradeDetail = components['schemas']['TradeDetailResponse']

export function formatTradeContextLabel(
  trade: TradeDetail,
  stockName: string | null,
): string {
  const sideLabel = trade.side === 'buy' ? '買い' : '売り'
  return `${trade.date} ${sideLabel} ${stockName ?? trade.symbol}`
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
