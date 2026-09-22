import { useMemo } from 'react'

import { FilterBar, type FilterOption } from '#components/runs/filter-bar'
import type { components } from '#lib/api/schema.gen'

type Strategy = components['schemas']['Strategy']
type Trade = components['schemas']['TradeListItem']

/** "all" もしくは戦略 ID。 */
export type StrategyFilter = string

export function StrategyFilterBar({
  trades,
  strategies,
  value,
  onChange,
  unlinkedCount,
  onlyUnlinked,
  onOnlyUnlinkedChange,
}: {
  trades: Trade[]
  strategies: Strategy[]
  value: StrategyFilter
  onChange: (v: StrategyFilter) => void
  unlinkedCount: number
  onlyUnlinked: boolean
  onOnlyUnlinkedChange: (value: boolean) => void
}) {
  const options = useMemo<FilterOption[]>(() => {
    const countByStrategy = new Map<string, number>()
    for (const t of trades) {
      countByStrategy.set(
        t.strategy_id,
        (countByStrategy.get(t.strategy_id) ?? 0) + 1,
      )
    }
    return strategies.map((s) => ({
      id: s.id,
      label: s.name,
      count: countByStrategy.get(s.id) ?? 0,
    }))
  }, [trades, strategies])

  return (
    <div className="flex flex-wrap items-center justify-between gap-2">
      <FilterBar
        options={options}
        value={value}
        onChange={onChange}
        allLabel="すべて"
        allCount={trades.length}
      />
      <button
        type="button"
        aria-pressed={onlyUnlinked}
        onClick={() => {
          onOnlyUnlinkedChange(!onlyUnlinked)
        }}
        className={`inline-flex items-center gap-1.5 border px-2.5 py-1 font-mono text-xs ${
          onlyUnlinked
            ? 'border-muted-foreground bg-surface-strong text-foreground'
            : 'border-border text-muted-foreground-strong hover:border-muted-foreground hover:text-foreground'
        }`}
      >
        <span>未紐付けのみ</span>
        <span className="text-2xs text-muted-foreground">{unlinkedCount}</span>
      </button>
    </div>
  )
}
