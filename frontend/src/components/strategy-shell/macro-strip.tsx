import { MACRO_MOCK, type MacroTick } from '@/lib/strategy-mock'

interface MacroStripProps {
  ticks?: MacroTick[]
}

// systemic risk を見逃さないため全ページ共通で薄く表示する。
export function MacroStrip({ ticks = MACRO_MOCK }: MacroStripProps) {
  return (
    <div className="flex items-stretch overflow-x-auto border-b border-[color:var(--color-border-strategy)] bg-[color:var(--color-bg-secondary)] font-mono [scrollbar-width:none] [&::-webkit-scrollbar]:hidden">
      <div className="flex flex-shrink-0 items-center gap-1.5 border-r border-[color:var(--color-border-strategy)] px-3.5 text-[11px] uppercase tracking-wider text-[color:var(--color-text-tertiary)]">
        <span className="font-bold text-[color:var(--color-accent-strategy)]">
          ##
        </span>
        <span>macro</span>
      </div>
      {ticks.map((t) => {
        const isUp = t.pct >= 0
        return (
          <div
            key={t.name}
            className="flex flex-shrink-0 items-baseline gap-2 border-r border-[color:var(--color-hairline)] px-4 py-1.5"
            title={t.name}
          >
            <span className="text-[11px] tracking-wide text-[color:var(--color-text-tertiary)]">
              {t.name}
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
      })}
    </div>
  )
}
