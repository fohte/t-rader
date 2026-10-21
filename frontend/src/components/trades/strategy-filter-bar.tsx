import { Checkbox } from '@fohte/ui/checkbox'
import { SegmentedControl } from '@fohte/ui/segmented-control'
import { useMemo } from 'react'

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
  const options = useMemo(() => {
    const countByStrategy = new Map<string, number>()
    for (const trade of trades) {
      countByStrategy.set(
        trade.strategy_id,
        (countByStrategy.get(trade.strategy_id) ?? 0) + 1,
      )
    }
    return [
      { value: 'all', label: 'すべて (' + String(trades.length) + ')' },
      ...strategies.map((strategy) => ({
        value: strategy.id,
        label:
          strategy.name +
          ' (' +
          String(countByStrategy.get(strategy.id) ?? 0) +
          ')',
      })),
    ]
  }, [trades, strategies])

  return (
    <div className="flex flex-wrap items-center justify-between gap-2">
      <SegmentedControl
        aria-label="戦略"
        value={value}
        options={options}
        onValueChange={onChange}
      />
      <label className="inline-flex items-center gap-2 font-mono text-xs text-muted-foreground-strong">
        <Checkbox
          aria-label="未紐付けのみ"
          checked={onlyUnlinked}
          onCheckedChange={(checked) => {
            onOnlyUnlinkedChange(checked)
          }}
        />
        <span>未紐付けのみ</span>
        <span className="text-2xs text-muted-foreground">{unlinkedCount}</span>
      </label>
    </div>
  )
}
