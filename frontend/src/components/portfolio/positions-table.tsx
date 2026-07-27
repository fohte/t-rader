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
      <div className="border border-[color:var(--color-border-strategy)] bg-[color:var(--panel)] px-4 py-8 text-center font-mono text-[12px] text-[color:var(--color-text-tertiary)]">
        保有ポジションなし
      </div>
    )
  }

  return (
    <div className="overflow-x-auto border border-[color:var(--color-border-strategy)] bg-[color:var(--panel)]">
      <table className="w-full min-w-[640px] font-mono text-[12px]">
        <thead>
          <tr className="border-b border-[color:var(--color-border-strategy)] text-[10px] uppercase tracking-wider text-[color:var(--color-text-tertiary)]">
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
                className="border-b border-[color:var(--color-hairline)] last:border-b-0 hover:bg-[color:var(--panel-inset)]"
              >
                <td className="px-3 py-2">
                  <div className="flex items-baseline gap-2">
                    <span className="text-[color:var(--color-text-primary)]">
                      {stockName ?? p.symbol}
                    </span>
                    {stockName != null && (
                      <span className="text-[10px] text-[color:var(--color-text-tertiary)]">
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
