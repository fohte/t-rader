import { Pencil, Trash2 } from 'lucide-react'
import { useMemo } from 'react'

import { formatYen, SOURCE_LABEL } from '@/components/trades/format'
import { Button } from '@/components/ui/button'
import type { components } from '@/lib/api/schema.gen'

type Trade = components['schemas']['Trade']
type Strategy = components['schemas']['Strategy']
type Stock = components['schemas']['Stock']

export function TradesTable({
  trades,
  strategies,
  stocks,
  showStrategy,
  onEdit,
  onDelete,
}: {
  trades: Trade[]
  strategies: Strategy[]
  stocks: Stock[]
  showStrategy: boolean
  onEdit: (t: Trade) => void
  onDelete: (t: Trade) => void
}) {
  const stockById = useMemo(
    () => new Map(stocks.map((s) => [s.id, s.name])),
    [stocks],
  )
  const strategyById = useMemo(
    () => new Map(strategies.map((s) => [s.id, s.name])),
    [strategies],
  )
  const sorted = useMemo(
    () => [...trades].sort((a, b) => b.date.localeCompare(a.date)),
    [trades],
  )

  if (trades.length === 0) {
    return (
      <div className="border border-[color:var(--color-border-strategy)] bg-[color:var(--panel)] px-4 py-8 text-center font-mono text-[12px] text-[color:var(--color-text-tertiary)]">
        取引がありません。「+ 取引を追加」から手入力できます。
      </div>
    )
  }

  return (
    <div className="overflow-x-auto border border-[color:var(--color-border-strategy)] bg-[color:var(--panel)]">
      <table className="w-full min-w-[820px] font-mono text-[12px]">
        <thead>
          <tr className="border-b border-[color:var(--color-border-strategy)] text-[10px] uppercase tracking-wider text-[color:var(--color-text-tertiary)]">
            <th className="px-3 py-2 text-left font-normal">日付</th>
            <th className="px-3 py-2 text-left font-normal">銘柄</th>
            <th className="px-3 py-2 text-center font-normal">売買</th>
            <th className="px-3 py-2 text-right font-normal">数量</th>
            <th className="px-3 py-2 text-right font-normal">単価</th>
            <th className="px-3 py-2 text-right font-normal">約定代金</th>
            {showStrategy && (
              <th className="px-3 py-2 text-left font-normal">戦略</th>
            )}
            <th className="px-3 py-2 text-center font-normal">入力</th>
            <th className="px-3 py-2 text-right font-normal">操作</th>
          </tr>
        </thead>
        <tbody>
          {sorted.map((t) => {
            const stockName = stockById.get(t.symbol)
            const strategyName = strategyById.get(t.strategy_id) ?? '-'
            return (
              <tr
                key={t.id}
                className="border-b border-[color:var(--color-hairline)] last:border-b-0 hover:bg-[color:var(--panel-inset)]"
              >
                <td className="px-3 py-2 tabular-nums text-[color:var(--color-text-secondary)]">
                  {t.date}
                </td>
                <td className="px-3 py-2">
                  <div className="flex items-baseline gap-2">
                    <span className="text-[color:var(--color-text-primary)]">
                      {stockName ?? t.symbol}
                    </span>
                    {stockName != null && (
                      <span className="text-[10px] text-[color:var(--color-text-tertiary)]">
                        {t.symbol}
                      </span>
                    )}
                  </div>
                </td>
                <td className="px-3 py-2 text-center">
                  <SideBadge side={t.side} />
                </td>
                <td className="px-3 py-2 text-right tabular-nums">
                  {t.qty.toLocaleString()}
                </td>
                <td className="px-3 py-2 text-right tabular-nums">
                  {formatYen(t.price)}
                </td>
                <td className="px-3 py-2 text-right tabular-nums">
                  {formatYen(t.qty * t.price)}
                </td>
                {showStrategy && (
                  <td className="px-3 py-2 text-[color:var(--color-text-secondary)]">
                    {strategyName}
                  </td>
                )}
                <td className="px-3 py-2 text-center text-[10px] text-[color:var(--color-text-tertiary)]">
                  {SOURCE_LABEL[t.source] ?? t.source}
                </td>
                <td className="px-3 py-2 text-right">
                  <div className="inline-flex gap-1">
                    <Button
                      size="icon-xs"
                      variant="ghost"
                      title="編集"
                      onClick={() => {
                        onEdit(t)
                      }}
                    >
                      <Pencil />
                    </Button>
                    <Button
                      size="icon-xs"
                      variant="ghost"
                      title="削除"
                      onClick={() => {
                        onDelete(t)
                      }}
                    >
                      <Trash2 />
                    </Button>
                  </div>
                </td>
              </tr>
            )
          })}
        </tbody>
      </table>
    </div>
  )
}

function SideBadge({ side }: { side: string }) {
  const isBuy = side === 'buy'
  const cls = isBuy
    ? 'border-[color:var(--color-up)] text-[color:var(--color-up)]'
    : 'border-[color:var(--color-down)] text-[color:var(--color-down)]'
  return (
    <span
      className={`inline-grid h-5 min-w-[24px] place-items-center border px-1 text-[10px] ${cls}`}
    >
      {isBuy ? '買' : '売'}
    </span>
  )
}
