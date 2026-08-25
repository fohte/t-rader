import { formatYen, pnlColorClass } from '#components/trades/format'

interface Stat {
  label: string
  value: string
  cls?: string
}

export function TradeStats({
  realizedPnl,
  feesTotal,
  tradeCount,
  openPositions,
}: {
  realizedPnl: number
  feesTotal: number
  tradeCount: number
  openPositions: number
}) {
  const stats: Stat[] = [
    {
      label: '実現損益 (累計)',
      value: formatYen(realizedPnl, true),
      cls: pnlColorClass(realizedPnl),
    },
    {
      label: '手数料 (累計)',
      value: formatYen(feesTotal),
      cls: 'text-muted-foreground-strong',
    },
    { label: '取引回数', value: tradeCount.toLocaleString() },
    { label: '保有銘柄', value: openPositions.toLocaleString() },
  ]

  return (
    <div className="grid grid-cols-2 gap-px overflow-hidden border border-border bg-border sm:grid-cols-4">
      {stats.map((s) => (
        <div
          key={s.label}
          className="flex flex-col gap-1 bg-card px-3.5 py-2.5"
        >
          <span className="font-mono text-[10px] uppercase tracking-wider text-muted-foreground">
            {s.label}
          </span>
          <span
            className={`font-mono text-lg font-bold tabular-nums ${
              s.cls ?? 'text-foreground'
            }`}
          >
            {s.value}
          </span>
        </div>
      ))}
    </div>
  )
}
