import { type MacroTick, useMacroTicks } from '@/lib/use-macro-ticks'

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
    <div className="flex items-stretch overflow-x-auto border-b border-[color:var(--color-border-strategy)] bg-[color:var(--color-bg-secondary)] font-mono [scrollbar-width:none] [&::-webkit-scrollbar]:hidden">
      <div className="flex flex-shrink-0 items-center gap-1.5 border-r border-[color:var(--color-border-strategy)] px-3.5 text-[11px] uppercase tracking-wider text-[color:var(--color-text-tertiary)]">
        <span className="font-bold text-[color:var(--color-accent-strategy)]">
          ##
        </span>
        <span>macro</span>
        {staleSince != null && ticks != null && (
          <span
            className="ml-1 border border-[color:var(--color-hairline)] px-1 text-[9px] text-[color:var(--color-text-tertiary)]"
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
      <div className="flex flex-shrink-0 items-center px-4 py-1.5 text-[11px] text-[color:var(--color-text-tertiary)]">
        loading…
      </div>
    )
  }
  if (ticks == null) {
    return (
      <div className="flex flex-shrink-0 items-center px-4 py-1.5 text-[11px] text-[color:var(--color-text-tertiary)]">
        N/A
      </div>
    )
  }
  return ticks.map((t) => {
    const isUp = t.pct >= 0
    return (
      <div
        key={t.symbol}
        className="flex flex-shrink-0 items-baseline gap-2 border-r border-[color:var(--color-hairline)] px-4 py-1.5"
        title={t.symbol}
      >
        <span className="text-[11px] tracking-wide text-[color:var(--color-text-tertiary)]">
          {t.symbol}
        </span>
        <span className="font-mono text-[13px] tabular-nums text-[color:var(--color-text-primary)]">
          {t.value}
        </span>
        <span
          className={`text-[11px] tabular-nums ${isUp ? 'text-[color:var(--color-up)]' : 'text-[color:var(--color-down)]'}`}
        >
          {isUp ? '+' : ''}
          {t.pct.toFixed(2)}%
        </span>
      </div>
    )
  })
}
