import { useMemo } from 'react'

import { StockNameLink } from '#components/stocks/stock-name-link'
import { formatYen, pnlColorClass } from '#components/trades/format'
import type { components } from '#lib/api/schema.gen'

type PositionSummary = components['schemas']['PositionSummary']
type Stock = components['schemas']['Stock']

export function PositionsTable({
  positions,
  stocks,
  displayMarketValue = false,
}: {
  positions: PositionSummary[]
  stocks: Stock[]
  displayMarketValue?: boolean
}) {
  const stockById = useMemo(
    () => new Map(stocks.map((s) => [s.id, s.name])),
    [stocks],
  )
  const sorted = useMemo(
    () =>
      [...positions].sort((a, b) => {
        if (!displayMarketValue) return b.cost_basis - a.cost_basis
        if (a.market_value == null) {
          return b.market_value == null ? a.symbol.localeCompare(b.symbol) : 1
        }
        if (b.market_value == null) return -1
        return (
          b.market_value - a.market_value || a.symbol.localeCompare(b.symbol)
        )
      }),
    [displayMarketValue, positions],
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
      <table className="w-full min-w-160 font-mono text-xs">
        <thead>
          <tr className="border-b border-border text-2xs uppercase tracking-wider text-muted-foreground">
            <th className="px-3 py-2 text-left font-normal">銘柄</th>
            {displayMarketValue ? (
              <>
                <th className="px-3 py-2 text-right font-normal">評価額</th>
                <th className="px-3 py-2 text-right font-normal">含み損益</th>
                <th className="px-3 py-2 text-right font-normal">数量</th>
                <th className="px-3 py-2 text-right font-normal">平均取得</th>
                <th className="px-3 py-2 text-right font-normal">取得簿価</th>
                <th className="px-3 py-2 text-right font-normal">現在値</th>
              </>
            ) : (
              <>
                <th className="px-3 py-2 text-right font-normal">数量</th>
                <th className="px-3 py-2 text-right font-normal">平均取得</th>
                <th className="px-3 py-2 text-right font-normal">取得簿価</th>
              </>
            )}
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
                    <StockNameLink
                      name={stockName ?? null}
                      symbol={p.symbol}
                      missingName="link-symbol"
                    />
                  </div>
                </td>
                {displayMarketValue ? (
                  <>
                    <td className="px-3 py-2 text-right tabular-nums">
                      {p.market_value == null ? '—' : formatYen(p.market_value)}
                    </td>
                    <td
                      className={`px-3 py-2 text-right tabular-nums ${p.unrealized_pnl == null ? 'text-muted-foreground-strong' : pnlColorClass(p.unrealized_pnl)}`}
                    >
                      {p.unrealized_pnl == null ? (
                        '—'
                      ) : (
                        <>
                          <span>{formatYen(p.unrealized_pnl, true)}</span>{' '}
                          <span className="block text-2xs">
                            {p.cost_basis <= 0
                              ? '—'
                              : `${p.unrealized_pnl > 0 ? '+' : ''}${((p.unrealized_pnl / p.cost_basis) * 100).toFixed(1)}%`}
                          </span>
                        </>
                      )}
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
                    <td className="px-3 py-2 text-right tabular-nums">
                      {p.current_price == null
                        ? '—'
                        : formatYen(p.current_price)}
                    </td>
                  </>
                ) : (
                  <>
                    <td className="px-3 py-2 text-right tabular-nums">
                      {p.qty.toLocaleString()}
                    </td>
                    <td className="px-3 py-2 text-right tabular-nums">
                      {formatYen(p.avg_cost)}
                    </td>
                    <td className="px-3 py-2 text-right tabular-nums">
                      {formatYen(p.cost_basis)}
                    </td>
                  </>
                )}
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
