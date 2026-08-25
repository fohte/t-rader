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
      className={`grid grid-cols-2 gap-px overflow-hidden border border-border bg-border ${cols}`}
    >
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
          {s.sub != null && (
            <span className="font-mono text-2xs tabular-nums text-muted-foreground">
              {s.sub}
            </span>
          )}
        </div>
      ))}
    </div>
  )
}
