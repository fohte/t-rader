import { Link } from '@tanstack/react-router'
import { useState } from 'react'

import { CreateHypothesisDialog } from '@/components/strategy-home/create-hypothesis-dialog'
import { HypothesisStatusPill } from '@/components/strategy-home/hypothesis-status-pill'
import { $api } from '@/lib/api/client'
import { formatRelative } from '@/lib/note-utils'

interface HypothesisListProps {
  strategyId: string
}

export function HypothesisList({ strategyId }: HypothesisListProps) {
  const [dialogOpen, setDialogOpen] = useState(false)
  const { data, isPending, isError } = $api.useQuery(
    'get',
    '/api/strategies/{id}/hypotheses',
    { params: { path: { id: strategyId } } },
  )
  const hypotheses = data ?? []

  return (
    <section className="border border-[color:var(--color-border-strategy)] bg-[color:var(--panel)]">
      <div className="flex items-center justify-between border-b border-[color:var(--color-hairline)] px-3.5 py-2">
        <h3 className="font-mono text-[12px] font-bold uppercase tracking-wider text-[color:var(--color-text-primary)]">
          仮説
        </h3>
        <button
          type="button"
          onClick={() => {
            setDialogOpen(true)
          }}
          className="font-mono text-[11px] text-[color:var(--color-text-tertiary)] hover:text-[color:var(--color-accent-strategy)]"
        >
          + 追加
        </button>
      </div>
      {isPending ? (
        <div className="px-3.5 py-3 font-mono text-[12px] text-[color:var(--color-text-tertiary)]">
          loading...
        </div>
      ) : isError ? (
        <div
          data-testid="hypothesis-list-error"
          className="px-3.5 py-3 font-mono text-[12px] text-[color:var(--color-accent-strategy)]"
        >
          仮説一覧の取得に失敗しました
        </div>
      ) : hypotheses.length === 0 ? (
        <div className="px-3.5 py-3 font-mono text-[12px] text-[color:var(--color-text-tertiary)]">
          —
        </div>
      ) : (
        <div>
          {hypotheses.map((h) => (
            <Link
              key={h.hypothesis_id}
              to="/strategies/$id/hypotheses/$hypothesisId"
              params={{ id: strategyId, hypothesisId: h.hypothesis_id }}
              className="flex flex-col gap-1 border-b border-[color:var(--color-hairline)] px-3.5 py-2.5 last:border-b-0 hover:bg-[color:var(--panel-inset)]"
            >
              <span className="line-clamp-2 text-[13px] text-[color:var(--color-text-primary)]">
                {h.title}
              </span>
              <span className="flex flex-wrap items-center gap-2 font-mono text-[11px]">
                <HypothesisStatusPill status={h.status} />
                <span className="ml-auto text-[color:var(--color-text-tertiary)]">
                  {formatRelative(h.updated_at)}
                </span>
              </span>
            </Link>
          ))}
        </div>
      )}
      <CreateHypothesisDialog
        strategyId={strategyId}
        open={dialogOpen}
        onOpenChange={setDialogOpen}
      />
    </section>
  )
}
