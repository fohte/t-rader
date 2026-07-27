import { useMemo } from 'react'

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
  const countByStrategy = useMemo(() => {
    const m = new Map<string, number>()
    for (const t of trades) {
      m.set(t.strategy_id, (m.get(t.strategy_id) ?? 0) + 1)
    }
    return m
  }, [trades])

  return (
    <div className="flex flex-wrap items-center gap-1.5">
      <FilterButton
        active={value === 'all'}
        onClick={() => {
          onChange('all')
        }}
        label="すべて"
        count={trades.length}
      />
      {strategies.map((s) => (
        <FilterButton
          key={s.id}
          active={value === s.id}
          onClick={() => {
            onChange(s.id)
          }}
          label={s.name}
          count={countByStrategy.get(s.id) ?? 0}
        />
      ))}
    </div>
  )
}

function FilterButton({
  active,
  onClick,
  label,
  count,
}: {
  active: boolean
  onClick: () => void
  label: string
  count: number
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      className={`inline-flex items-center gap-1.5 border px-2.5 py-1 font-mono text-[12px] ${
        active
          ? 'border-[color:var(--color-text-tertiary)] bg-[color:var(--panel-inset)] text-[color:var(--color-text-primary)]'
          : 'border-[color:var(--color-border-strategy)] text-[color:var(--color-text-secondary)] hover:border-[color:var(--color-text-tertiary)] hover:text-[color:var(--color-text-primary)]'
      }`}
    >
      <span>{label}</span>
      <span className="text-[10px] text-[color:var(--color-text-tertiary)]">
        {count}
      </span>
    </button>
  )
}
