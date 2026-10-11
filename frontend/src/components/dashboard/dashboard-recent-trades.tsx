import { Link } from '@tanstack/react-router'

import { StockNameLink } from '#components/stocks/stock-name-link'
import { formatYen } from '#components/trades/format'
import { Skeleton } from '#components/ui/skeleton'
import type { components } from '#lib/api/schema.gen'

type Stock = components['schemas']['Stock']
type Trade = components['schemas']['TradeListItem']

function SideBadge({ side }: { side: string }) {
  const isBuy = side === 'buy'
  return (
    <span
      className={`inline-grid h-5 min-w-6 place-items-center border px-1 text-2xs ${
        isBuy ? 'border-up text-up' : 'border-down text-down'
      }`}
    >
      {isBuy ? '買' : '売'}
    </span>
  )
}

export function DashboardRecentTrades({
  trades,
  stocks,
  onReflect,
  isError,
}: {
  trades: Trade[] | null
  stocks: Stock[]
  onReflect: (trade: Trade) => void
  isError: boolean
}) {
  const recentTrades =
    trades == null
      ? []
      : [...trades]
          .sort(
            (left, right) =>
              right.date.localeCompare(left.date) ||
              right.created_at.localeCompare(left.created_at),
          )
          .slice(0, 4)
  const stockById = new Map(stocks.map((stock) => [stock.id, stock.name]))

  return (
    <section className="space-y-2" aria-labelledby="dashboard-trades-title">
      <div className="flex items-baseline justify-between gap-3">
        <h2
          id="dashboard-trades-title"
          className="font-mono text-2xs uppercase tracking-wider text-muted-foreground"
        >
          <span className="mr-2 text-primary">&gt;</span>
          直近の取引
        </h2>
        <Link
          to="/trades"
          className="shrink-0 font-mono text-2xs text-primary hover:underline"
        >
          取引をすべて見る →
        </Link>
      </div>
      <div className="border border-border bg-card">
        {isError ? (
          <p className="px-4 py-5 font-mono text-xs text-muted-foreground">
            取引を読み込めませんでした。
          </p>
        ) : trades == null ? (
          <div className="space-y-2 p-3">
            <Skeleton className="h-10 w-full" />
            <Skeleton className="h-10 w-full" />
          </div>
        ) : recentTrades.length === 0 ? (
          <p className="px-4 py-7 text-center font-mono text-xs text-muted-foreground">
            取引はまだありません。
          </p>
        ) : (
          <>
            <div className="hidden overflow-x-auto md:block">
              <table className="w-full min-w-160 font-mono text-xs">
                <thead>
                  <tr className="border-b border-border text-2xs uppercase tracking-wider text-muted-foreground">
                    <th className="px-3 py-2 text-left font-normal">日付</th>
                    <th className="px-3 py-2 text-left font-normal">銘柄</th>
                    <th className="px-3 py-2 text-center font-normal">売買</th>
                    <th className="px-3 py-2 text-right font-normal">数量</th>
                    <th className="px-3 py-2 text-right font-normal">単価</th>
                    <th className="px-3 py-2 text-right font-normal">
                      振り返り
                    </th>
                  </tr>
                </thead>
                <tbody>
                  {recentTrades.map((trade) => (
                    <tr
                      key={trade.id}
                      className="border-b border-border last:border-b-0 hover:bg-surface-strong"
                    >
                      <td className="whitespace-nowrap px-3 py-2 tabular-nums text-muted-foreground-strong">
                        {trade.date}
                      </td>
                      <td className="whitespace-nowrap px-3 py-2">
                        <StockNameLink
                          name={stockById.get(trade.symbol) ?? null}
                          symbol={trade.symbol}
                          missingName="link-symbol"
                        />
                      </td>
                      <td className="px-3 py-2 text-center">
                        <SideBadge side={trade.side} />
                      </td>
                      <td className="px-3 py-2 text-right tabular-nums">
                        {trade.qty.toLocaleString()}
                      </td>
                      <td className="px-3 py-2 text-right tabular-nums">
                        {formatYen(trade.price)}
                      </td>
                      <td className="px-3 py-2 text-right">
                        <button
                          type="button"
                          onClick={() => {
                            onReflect(trade)
                          }}
                          className="whitespace-nowrap font-mono text-xs text-primary hover:underline"
                        >
                          振り返る →
                        </button>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
            <ul className="divide-y divide-border md:hidden">
              {recentTrades.map((trade) => (
                <li
                  key={trade.id}
                  className="flex min-w-0 items-center gap-2 px-3 py-2.5"
                >
                  <div className="min-w-0 flex-1 space-y-1">
                    <div className="flex min-w-0 items-center gap-2">
                      <SideBadge side={trade.side} />
                      <span className="min-w-0 truncate">
                        <StockNameLink
                          name={stockById.get(trade.symbol) ?? null}
                          symbol={trade.symbol}
                          missingName="link-symbol"
                        />
                      </span>
                    </div>
                    <p className="font-mono text-2xs tabular-nums text-muted-foreground">
                      {trade.date} · {trade.qty.toLocaleString()} 株 ·{' '}
                      {formatYen(trade.price)}
                    </p>
                  </div>
                  <button
                    type="button"
                    onClick={() => {
                      onReflect(trade)
                    }}
                    className="whitespace-nowrap font-mono text-2xs text-primary hover:underline"
                  >
                    振り返る →
                  </button>
                </li>
              ))}
            </ul>
          </>
        )}
      </div>
    </section>
  )
}
