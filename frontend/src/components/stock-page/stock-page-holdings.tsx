import { formatYen, pnlColorClass } from '#components/trades/format'
import { Skeleton } from '#components/ui/skeleton'
import type { components } from '#lib/api/schema.gen'
import { cn } from '#lib/utils'

type Position = components['schemas']['PositionSummary']

export function StockPageHoldings({
  position,
  isLoading = false,
  hasError = false,
}: {
  position: Position | null
  isLoading?: boolean
  hasError?: boolean
}) {
  if (position == null && !isLoading && !hasError) return null

  const values =
    position == null || position.qty <= 0
      ? []
      : [
          {
            label: '数量',
            value: `${position.qty.toLocaleString('ja-JP')} 株`,
          },
          { label: '平均取得', value: formatYen(position.avg_cost) },
          { label: '取得簿価', value: formatYen(position.cost_basis) },
          {
            label: '実現損益',
            value: formatYen(position.realized_pnl, true),
            className: pnlColorClass(position.realized_pnl),
          },
        ]

  return (
    <section className="border border-border bg-card" aria-label="保有">
      <div className="border-b border-border px-3.5 py-2">
        <h2 className="font-mono text-xs font-bold uppercase tracking-wider text-foreground">
          <span className="mr-2 text-primary">&gt;</span>
          保有
        </h2>
      </div>
      {isLoading ? (
        <Skeleton className="h-40 w-full" />
      ) : hasError ? (
        <p className="px-3.5 py-4 font-mono text-xs text-destructive">
          保有情報の取得に失敗しました
        </p>
      ) : (
        <dl className="grid grid-cols-2 sm:grid-cols-1 sm:gap-y-1 sm:p-3.5">
          {values.map((item, index) => (
            <div
              key={item.label}
              className={cn(
                'flex min-w-0 flex-col gap-1 px-3 py-2',
                index % 2 === 0 && 'border-r border-border',
                index < 2 && 'border-b border-border',
                'sm:flex-row sm:items-baseline sm:justify-between sm:gap-3 sm:border-0 sm:px-0 sm:py-0',
              )}
            >
              <dt className="font-mono text-2xs text-muted-foreground">
                {item.label}
              </dt>
              <dd
                className={`truncate font-mono text-sm tabular-nums ${item.className ?? ''}`}
              >
                {item.value}
              </dd>
            </div>
          ))}
        </dl>
      )}
    </section>
  )
}
