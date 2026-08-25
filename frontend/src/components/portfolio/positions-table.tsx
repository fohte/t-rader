import { useMemo } from 'react'

import { formatYen, pnlColorClass } from '#components/trades/format'
import type { components } from '#lib/api/schema.gen'

type PositionSummary = components['schemas']['PositionSummary']
type Stock = components['schemas']['Stock']

export function PositionsTable({
  positions,
  stocks,
}: {
  positions: PositionSummary[]
  stocks: Stock[]
}) {
  const stockById = useMemo(
    () => new Map(stocks.map((s) => [s.id, s.name])),
    [stocks],
  )
  const sorted = useMemo(
    () => [...positions].sort((a, b) => b.cost_basis - a.cost_basis),
    [positions],
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
      <table className="w-full min-w-[640px] font-mono text-xs">
        <thead>
          <tr className="border-b border-border text-[10px] uppercase tracking-wider text-muted-foreground">
            <th className="px-3 py-2 text-left font-normal">銘柄</th>
            <th className="px-3 py-2 text-right font-normal">数量</th>
            <th className="px-3 py-2 text-right font-normal">平均取得</th>
            <th className="px-3 py-2 text-right font-normal">取得簿価</th>
            <th className="px-3 py-2 text-right font-normal">実現損益</th>
          </tr>
        </thead>
        <tbody>
          {sorted.map((p) => {
            const stockName = stockById.get(p.symbol)
            return (
              <tr
                key={p.symbol}
                className="border-b border-border last:border-b-0 hover:bg-surface-strong"
              >
                <td className="px-3 py-2">
                  <div className="flex items-baseline gap-2">
                    <span className="text-foreground">
                      {stockName ?? p.symbol}
                    </span>
                    {stockName != null && (
                      <span className="text-[10px] text-muted-foreground">
                        {p.symbol}
                      </span>
                    )}
                  </div>
                </td>
                <td className="px-3 py-2 text-right tabular-nums">
                  {p.qty.toLocaleString()}
                </td>
                <td className="px-3 py-2 text-right tabular-nums">
                  {formatYen(p.avg_cost)}
                </td>
                <td className="px-3 py-2 text-right tabular-nums">
                  {formatYen(p.cost_basis)}
                </td>
                <td
                  className={`px-3 py-2 text-right tabular-nums ${pnlColorClass(p.realized_pnl)}`}
                >
                  {formatYen(p.realized_pnl, true)}
                </td>
              </tr>
            )
          })}
        </tbody>
      </table>
    </div>
  )
}
