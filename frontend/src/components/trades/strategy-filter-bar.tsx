import { useMemo } from 'react'

import { FilterBar, type FilterOption } from '#components/runs/filter-bar'
import type { components } from '#lib/api/schema.gen'

type Strategy = components['schemas']['Strategy']
type Trade = components['schemas']['Trade']

/** "all" もしくは戦略 ID。 */
export type StrategyFilter = string

export function StrategyFilterBar({
  trades,
  strategies,
  value,
  onChange,
}: {
  trades: Trade[]
  strategies: Strategy[]
  value: StrategyFilter
  onChange: (v: StrategyFilter) => void
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
    <FilterBar
      options={options}
      value={value}
      onChange={onChange}
      allLabel="すべて"
      allCount={trades.length}
    />
  )
}
