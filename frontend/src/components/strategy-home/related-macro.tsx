import { MACRO_MOCK } from '@/lib/strategy-mock'

interface RelatedMacroProps {
  // 戦略の関心から抽出した indicator id 一覧
  indicatorIds: string[]
}

export function RelatedMacro({ indicatorIds }: RelatedMacroProps) {
  const items = indicatorIds
    .map((id) => MACRO_MOCK.find((m) => normalize(m.name) === normalize(id)))
    .filter((m): m is (typeof MACRO_MOCK)[number] => m != null)

  return (
    <section className="border border-[color:var(--color-border-strategy)] bg-[color:var(--panel)]">
      <div className="flex items-baseline justify-between border-b border-[color:var(--color-hairline)] px-3.5 py-2">
        <h3 className="font-mono text-[12px] font-bold uppercase tracking-wider text-[color:var(--color-text-primary)]">
          戦略関連マクロ
        </h3>
      </div>
      {items.length === 0 ? (
        <div className="px-3.5 py-3 font-mono text-[12px] text-[color:var(--color-text-tertiary)]">
          —
        </div>
      ) : (
        <div className="grid grid-cols-2 gap-px bg-[color:var(--color-hairline)]">
          {items.map((m) => {
            const isUp = m.pct >= 0
            return (
              <div
                key={m.name}
                className="flex flex-col gap-0.5 bg-[color:var(--panel)] px-3 py-2.5"
              >
                <div className="font-mono text-[10px] uppercase tracking-wider text-[color:var(--color-text-tertiary)]">
                  {m.name}
                </div>
                <div className="font-mono text-[14px] tabular-nums text-[color:var(--color-text-primary)]">
                  {m.value}
                </div>
                <div
                  className={`font-mono text-[11px] tabular-nums ${isUp ? 'text-[color:var(--color-up)]' : 'text-[color:var(--color-down)]'}`}
                >
                  {isUp ? '▲' : '▼'} {Math.abs(m.pct).toFixed(2)}%
                </div>
              </div>
            )
          })}
        </div>
      )}
    </section>
  )
}

function normalize(s: string): string {
  return s.replace(/[\s/]/g, '').toUpperCase()
}
