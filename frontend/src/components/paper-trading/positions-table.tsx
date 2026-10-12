import { formatYen, pnlColorClass } from '#components/trades/format'
import type { components } from '#lib/api/schema.gen'

type PaperTradePosition = components['schemas']['PaperTradePosition']

export function PaperTradePositionsTable({
  positions,
  stockNames,
}: {
  positions: PaperTradePosition[]
  stockNames: Record<string, string>
}) {
  const sorted = [...positions].sort(
    (left, right) => right.market_value_jpy - left.market_value_jpy,
  )

  if (sorted.length === 0) {
    return (
      <div className="border border-border bg-card px-4 py-8 text-center font-mono text-xs text-muted-foreground">
        保有ポジションなし
      </div>
    )
  }

  return (
    <div className="overflow-x-auto border border-border bg-card">
      <table className="w-full min-w-180 font-mono text-xs">
        <thead>
          <tr className="border-b border-border text-2xs uppercase tracking-wider text-muted-foreground">
            <th className="px-3 py-2 text-left font-normal">銘柄</th>
            <th className="px-3 py-2 text-right font-normal">株数</th>
            <th className="px-3 py-2 text-right font-normal">平均取得</th>
            <th className="px-3 py-2 text-right font-normal">現在値</th>
            <th className="px-3 py-2 text-right font-normal">評価額</th>
            <th className="px-3 py-2 text-right font-normal">含み損益</th>
          </tr>
        </thead>
        <tbody>
          {sorted.map((position) => {
            const stockName = stockNames[position.stock_id]
            return (
              <tr
                key={position.stock_id}
                className="border-b border-border last:border-b-0 hover:bg-surface-strong"
              >
                <td className="px-3 py-2">
                  <div className="flex items-baseline gap-2">
                    <span className="text-foreground">
                      {stockName ?? position.stock_id}
                    </span>
                    {stockName != null && (
                      <span className="text-2xs text-muted-foreground">
                        {position.stock_id}
                      </span>
                    )}
                  </div>
                </td>
                <td className="px-3 py-2 text-right tabular-nums">
                  {position.qty.toLocaleString()}
                </td>
                <td className="px-3 py-2 text-right tabular-nums">
                  {formatYen(position.avg_cost_jpy)}
                </td>
                <td className="px-3 py-2 text-right tabular-nums">
                  {formatYen(position.current_price_jpy)}
                </td>
                <td className="px-3 py-2 text-right tabular-nums">
                  {formatYen(position.market_value_jpy)}
                </td>
                <td
                  className={
                    'px-3 py-2 text-right tabular-nums ' +
                    pnlColorClass(position.unrealized_pnl_jpy)
                  }
                >
                  {formatYen(position.unrealized_pnl_jpy, true)}
                </td>
              </tr>
            )
          })}
        </tbody>
      </table>
    </div>
  )
}
