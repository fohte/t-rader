export interface StatItem {
  label: string
  value: string
  sub?: string
  cls?: string
}

export function StatRow({ stats }: { stats: StatItem[] }) {
  const cols =
    stats.length >= 5
      ? 'sm:grid-cols-3 lg:grid-cols-5'
      : stats.length === 4
        ? 'sm:grid-cols-2 lg:grid-cols-4'
        : 'sm:grid-cols-3'
  return (
    <div
      className={`grid grid-cols-2 gap-px overflow-hidden border border-[color:var(--color-border-strategy)] bg-[color:var(--color-border-strategy)] ${cols}`}
    >
      {stats.map((s) => (
        <div
          key={s.label}
          className="flex flex-col gap-1 bg-[color:var(--panel)] px-3.5 py-2.5"
        >
          <span className="font-mono text-[10px] uppercase tracking-wider text-[color:var(--color-text-tertiary)]">
            {s.label}
          </span>
          <span
            className={`font-mono text-[18px] font-bold tabular-nums ${
              s.cls ?? 'text-[color:var(--color-text-primary)]'
            }`}
          >
            {s.value}
          </span>
          {s.sub != null && (
            <span className="font-mono text-[11px] tabular-nums text-[color:var(--color-text-tertiary)]">
              {s.sub}
            </span>
          )}
        </div>
      ))}
    </div>
  )
}
