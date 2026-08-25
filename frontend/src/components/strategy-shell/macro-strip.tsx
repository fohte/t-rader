import { type MacroTick, useMacroTicks } from '#lib/use-macro-ticks'

export interface MacroStripViewProps {
  ticks: MacroTick[] | null
  staleSince: string | null
  isPending: boolean
}

// systemic risk を見逃さないため全ページ共通で薄く表示する。
export function MacroStrip() {
  const { ticks, staleSince, isPending } = useMacroTicks()
  return (
    <MacroStripView
      ticks={ticks}
      staleSince={staleSince}
      isPending={isPending}
    />
  )
}

export function MacroStripView({
  ticks,
  staleSince,
  isPending,
}: MacroStripViewProps) {
  return (
    <div className="flex items-stretch overflow-x-auto border-b border-border bg-bg-secondary font-mono [scrollbar-width:none] [&::-webkit-scrollbar]:hidden">
      <div className="flex flex-shrink-0 items-center gap-1.5 border-r border-border px-3.5 text-2xs uppercase tracking-wider text-muted-foreground">
        <span className="font-bold text-primary">##</span>
        <span>macro</span>
        {staleSince != null && ticks != null && (
          <span
            className="ml-1 border border-border px-1 text-[9px] text-muted-foreground"
            title={`最終更新失敗 since ${staleSince}`}
          >
            stale
          </span>
        )}
      </div>
      {renderBody({ ticks, isPending })}
    </div>
  )
}

function renderBody({
  ticks,
  isPending,
}: {
  ticks: MacroTick[] | null
  isPending: boolean
}) {
  if (isPending && ticks == null) {
    return (
      <div className="flex flex-shrink-0 items-center px-4 py-1.5 text-2xs text-muted-foreground">
        loading…
      </div>
    )
  }
  if (ticks == null) {
    return (
      <div className="flex flex-shrink-0 items-center px-4 py-1.5 text-2xs text-muted-foreground">
        N/A
      </div>
    )
  }
  return ticks.map((t) => {
    const isUp = t.pct >= 0
    return (
      <div
        key={t.symbol}
        className="flex flex-shrink-0 items-baseline gap-2 border-r border-border px-4 py-1.5"
        title={t.symbol}
      >
        <span className="text-2xs tracking-wide text-muted-foreground">
          {t.symbol}
        </span>
        <span className="font-mono text-[13px] tabular-nums text-foreground">
          {t.value}
        </span>
        <span
          className={`text-2xs tabular-nums ${isUp ? 'text-up' : 'text-down'}`}
        >
          {isUp ? '+' : ''}
          {t.pct.toFixed(2)}%
        </span>
      </div>
    )
  })
}
