import { type MacroTick, useMacroTicks } from '#lib/use-macro-ticks'

interface RelatedMacroProps {
  // 戦略の関心から抽出した indicator id 一覧
  indicatorIds: string[]
}

export interface RelatedMacroViewProps extends RelatedMacroProps {
  ticks: MacroTick[] | null
  staleSince: string | null
}

export function RelatedMacro({ indicatorIds }: RelatedMacroProps) {
  const { ticks, staleSince } = useMacroTicks()
  return (
    <RelatedMacroView
      indicatorIds={indicatorIds}
      ticks={ticks}
      staleSince={staleSince}
    />
  )
}

export function RelatedMacroView({
  indicatorIds,
  ticks,
  staleSince,
}: RelatedMacroViewProps) {
  const items =
    ticks == null
      ? []
      : indicatorIds
          .map((id) => ticks.find((m) => normalize(m.symbol) === normalize(id)))
          .filter((m): m is MacroTick => m != null)

  return (
    <section className="border border-border bg-card">
      <div className="flex items-baseline justify-between border-b border-border px-3.5 py-2">
        <h3 className="font-mono text-xs font-bold uppercase tracking-wider text-foreground">
          戦略関連マクロ
        </h3>
        {staleSince != null && ticks != null && (
          <span className="font-mono text-[10px] uppercase tracking-wider text-muted-foreground">
            stale
          </span>
        )}
      </div>
      {renderBody(items, ticks == null)}
    </section>
  )
}

function renderBody(items: MacroTick[], unavailable: boolean) {
  if (unavailable) {
    return (
      <div className="px-3.5 py-3 font-mono text-xs text-muted-foreground">
        N/A
      </div>
    )
  }
  if (items.length === 0) {
    return (
      <div className="px-3.5 py-3 font-mono text-xs text-muted-foreground">
        —
      </div>
    )
  }
  return (
    <div className="grid grid-cols-2 gap-px bg-border">
      {items.map((m) => {
        const isUp = m.pct >= 0
        return (
          <div
            key={m.symbol}
            className="flex flex-col gap-0.5 bg-card px-3 py-2.5"
          >
            <div className="font-mono text-[10px] uppercase tracking-wider text-muted-foreground">
              {m.symbol}
            </div>
            <div className="font-mono text-sm tabular-nums text-foreground">
              {m.value}
            </div>
            <div
              className={`font-mono text-2xs tabular-nums ${isUp ? 'text-up' : 'text-down'}`}
            >
              {isUp ? '▲' : '▼'} {Math.abs(m.pct).toFixed(2)}%
            </div>
          </div>
        )
      })}
    </div>
  )
}

function normalize(s: string): string {
  return s.replace(/[\s/]/g, '').toUpperCase()
}
