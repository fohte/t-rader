import { Link } from '@tanstack/react-router'
import { useState } from 'react'

import { CreateHypothesisDialog } from '#components/strategy-home/create-hypothesis-dialog'
import { HypothesisStatusPill } from '#components/strategy-home/hypothesis-status-pill'
import { $api } from '#lib/api/client'
import { formatRelative } from '#lib/note-utils'

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
    <section className="border border-border bg-card">
      <div className="flex items-center justify-between border-b border-border px-3.5 py-2">
        <h3 className="font-mono text-xs font-bold uppercase tracking-wider text-foreground">
          仮説
        </h3>
        <button
          type="button"
          onClick={() => {
            setDialogOpen(true)
          }}
          className="font-mono text-2xs text-muted-foreground hover:text-primary"
        >
          + 追加
        </button>
      </div>
      {isPending ? (
        <div className="px-3.5 py-3 font-mono text-xs text-muted-foreground">
          loading...
        </div>
      ) : isError ? (
        <div
          data-testid="hypothesis-list-error"
          className="px-3.5 py-3 font-mono text-xs text-primary"
        >
          仮説一覧の取得に失敗しました
        </div>
      ) : hypotheses.length === 0 ? (
        <div className="px-3.5 py-3 font-mono text-xs text-muted-foreground">
          —
        </div>
      ) : (
        <div>
          {hypotheses.map((h) => (
            <Link
              key={h.hypothesis_id}
              to="/strategies/$id/hypotheses/$hypothesisId"
              params={{ id: strategyId, hypothesisId: h.hypothesis_id }}
              className="flex flex-col gap-1 border-b border-border px-3.5 py-2.5 last:border-b-0 hover:bg-surface-strong"
            >
              <span className="line-clamp-2 text-[13px] text-foreground">
                {h.title}
              </span>
              <span className="flex flex-wrap items-center gap-2 font-mono text-2xs">
                <HypothesisStatusPill status={h.status} />
                <span className="ml-auto text-muted-foreground">
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
