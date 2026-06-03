import { createFileRoute, Link } from '@tanstack/react-router'

import { RefChip } from '@/components/strategy-shell/ref-chip'
import { STRATEGIES_MOCK } from '@/lib/strategy-mock'

export const Route = createFileRoute('/strategies/')({
  component: StrategyListPage,
})

function StrategyListPage() {
  return (
    <div className="font-sans text-[color:var(--color-text-primary)]">
      <div className="max-w-[720px] pb-6 pt-4">
        <h1 className="mb-3 text-[26px] font-bold tracking-tight">
          <span className="font-mono font-bold text-[color:var(--color-accent-strategy)]">
            #
          </span>{' '}
          戦略
        </h1>
        <p className="text-[15px] leading-relaxed text-[color:var(--color-text-secondary)]">
          各戦略は永続ワークスペースとして、シード関心と LLM 産出物
          (ノート/アノテーション/コメント) を保持します。
        </p>
      </div>
      <div className="grid grid-cols-1 gap-3.5 sm:grid-cols-2 lg:grid-cols-3">
        {STRATEGIES_MOCK.map((s) => (
          <Link
            key={s.id}
            to="/strategies/$id"
            params={{ id: s.id }}
            className="flex min-h-[188px] cursor-pointer flex-col gap-3.5 border border-[color:var(--color-border-strategy)] bg-[color:var(--panel)] p-4 transition-colors hover:border-[color:var(--color-text-tertiary)]"
          >
            <div className="flex items-start justify-between gap-2.5">
              <div>
                <div className="font-mono text-[16px] font-bold leading-tight">
                  {s.name}
                </div>
                <div className="mt-1 font-mono text-[11px] tracking-wide text-[color:var(--color-text-tertiary)]">
                  {s.horizon}
                </div>
              </div>
              {s.unread > 0 && (
                <span className="inline-grid h-4 min-w-[18px] place-items-center bg-[color:var(--color-accent-strategy)] px-1 font-mono text-[10px] text-white">
                  {s.unread}
                </span>
              )}
            </div>
            <p className="text-[13px] leading-relaxed text-[color:var(--color-text-secondary)]">
              {s.desc}
            </p>
            <div className="mt-auto flex items-center justify-between border-t border-[color:var(--color-hairline)] pt-3 font-mono text-[11px]">
              <span className="text-[color:var(--color-text-tertiary)]">
                {s.updatedAt}
              </span>
              <span className="text-[color:var(--color-text-secondary)]">
                {s.unread > 0 ? (
                  <>
                    <span className="font-bold text-[color:var(--color-accent-strategy)]">
                      {s.unread}
                    </span>{' '}
                    未読
                  </>
                ) : (
                  'すべて既読'
                )}
              </span>
            </div>
          </Link>
        ))}
      </div>
      <div className="mt-4 text-[12px] text-[color:var(--color-text-tertiary)]">
        サンプル: <RefChip token="stock:7203" />
        {' / '}
        <RefChip token="indicator:USDJPY" />
      </div>
    </div>
  )
}
