export interface FilterOption {
  id: string
  label: string
  count: number
}

export function FilterBar({
  options,
  value,
  onChange,
  allLabel,
  allCount,
}: {
  options: FilterOption[]
  value: string
  onChange: (v: string) => void
  allLabel: string
  allCount: number
}) {
  return (
    <div className="flex flex-wrap items-center gap-1.5">
      <FilterButton
        active={value === 'all'}
        onClick={() => {
          onChange('all')
        }}
        label={allLabel}
        count={allCount}
      />
      {options.map((o) => (
        <FilterButton
          key={o.id}
          active={value === o.id}
          onClick={() => {
            onChange(o.id)
          }}
          label={o.label}
          count={o.count}
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
      className={`inline-flex items-center gap-1.5 border px-2.5 py-1 font-mono text-xs ${
        active
          ? 'border-muted-foreground bg-surface-strong text-foreground'
          : 'border-border text-muted-foreground-strong hover:border-muted-foreground hover:text-foreground'
      }`}
    >
      <span>{label}</span>
      <span className="text-2xs text-muted-foreground">{count}</span>
    </button>
  )
}
