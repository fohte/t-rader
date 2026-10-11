import { Link } from '@tanstack/react-router'

import { formatDateTime } from '#components/paper-trading/format'
import { formatYen } from '#components/trades/format'
import type { components } from '#lib/api/schema.gen'

type PaperTradeOrder = components['schemas']['PaperTradeOrder']

function outcomeLabel(outcome: string | null | undefined): string {
  if (outcome === 'filled') return '約定'
  if (outcome === 'rejected') return '却下'
  if (outcome == null) return '未約定'
  return '不明'
}

export function PaperTradeOrdersTable({
  orders,
  stockNames,
}: {
  orders: PaperTradeOrder[]
  stockNames: Record<string, string>
}) {
  const sorted = [...orders].sort((left, right) =>
    right.ordered_at.localeCompare(left.ordered_at),
  )

  if (sorted.length === 0) {
    return (
      <div className="border border-border bg-card px-4 py-8 text-center font-mono text-xs text-muted-foreground">
        注文はありません。
      </div>
    )
  }

  return (
    <div className="overflow-x-auto border border-border bg-card">
      <table className="w-full min-w-220 font-mono text-xs">
        <thead>
          <tr className="border-b border-border text-2xs uppercase tracking-wider text-muted-foreground">
            <th className="px-3 py-2 text-left font-normal">注文日時</th>
            <th className="px-3 py-2 text-left font-normal">銘柄</th>
            <th className="px-3 py-2 text-center font-normal">売買</th>
            <th className="px-3 py-2 text-right font-normal">株数</th>
            <th className="px-3 py-2 text-center font-normal">結果</th>
            <th className="px-3 py-2 text-left font-normal">
              約定日 / 価格・却下理由
            </th>
            <th className="px-3 py-2 text-center font-normal">根拠</th>
          </tr>
        </thead>
        <tbody>
          {sorted.map((order) => {
            const stockName = stockNames[order.stock_id]
            const outcome = outcomeLabel(order.outcome)
            const outcomeColor =
              order.outcome === 'filled'
                ? 'text-up'
                : order.outcome === 'rejected'
                  ? 'text-down'
                  : 'text-muted-foreground-strong'
            const sideLabel =
              order.side === 'buy'
                ? '買'
                : order.side === 'sell'
                  ? '売'
                  : order.side
            const sideColor =
              order.side === 'buy'
                ? 'text-up'
                : order.side === 'sell'
                  ? 'text-down'
                  : 'text-muted-foreground-strong'

            return (
              <tr
                key={order.order_id}
                className="border-b border-border last:border-b-0 hover:bg-surface-strong"
              >
                <td className="px-3 py-2 tabular-nums text-muted-foreground-strong">
                  {formatDateTime(order.ordered_at)}
                </td>
                <td className="px-3 py-2">
                  <div className="flex items-baseline gap-2">
                    <span className="text-foreground">
                      {stockName ?? order.stock_id}
                    </span>
                    {stockName != null && (
                      <span className="text-2xs text-muted-foreground">
                        {order.stock_id}
                      </span>
                    )}
                  </div>
                </td>
                <td className={'px-3 py-2 text-center ' + sideColor}>
                  {sideLabel}
                </td>
                <td className="px-3 py-2 text-right tabular-nums">
                  {order.qty.toLocaleString()}
                </td>
                <td className={'px-3 py-2 text-center ' + outcomeColor}>
                  {outcome}
                </td>
                <td className="px-3 py-2 text-muted-foreground-strong">
                  {order.outcome === 'filled' &&
                  order.fill_date != null &&
                  order.fill_price_jpy != null
                    ? order.fill_date + ' / ' + formatYen(order.fill_price_jpy)
                    : order.outcome === 'rejected'
                      ? (order.reject_reason ?? '却下理由なし')
                      : '—'}
                </td>
                <td className="px-3 py-2 text-center">
                  <Link
                    to="/notes/$noteId"
                    params={{ noteId: order.note_id }}
                    search={{ version_id: order.note_version_id }}
                    title="注文時点のノートバージョンを開く"
                    className="text-primary hover:underline"
                  >
                    ノートのバージョン →
                  </Link>
                </td>
              </tr>
            )
          })}
        </tbody>
      </table>
    </div>
  )
}
