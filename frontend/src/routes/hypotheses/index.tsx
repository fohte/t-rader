import { createFileRoute, Link } from '@tanstack/react-router'
import { useState } from 'react'

import { StrategyFilterSelect } from '#components/strategy-filter-select'
import { CreateHypothesisDialog } from '#components/strategy-home/create-hypothesis-dialog'
import { HypothesisStatusPill } from '#components/strategy-home/hypothesis-status-pill'
import { Skeleton } from '#components/ui/skeleton'
import { $api } from '#lib/api/client'
import { formatRelative } from '#lib/note-utils'

export const Route = createFileRoute('/hypotheses/')({
  validateSearch: (
    search: Record<string, unknown>,
  ): { strategy_id?: string } => ({
    strategy_id:
      typeof search.strategy_id === 'string' ? search.strategy_id : undefined,
  }),
  component: HypothesesPage,
})

function HypothesesPage() {
  const { strategy_id } = Route.useSearch()
  const navigate = Route.useNavigate()
  const [creating, setCreating] = useState(false)

  const { data: hypotheses, isPending } = $api.useQuery(
    'get',
    '/api/hypotheses',
    { params: { query: { strategy_id } } },
  )

  return (
    <div className="font-sans text-foreground">
      <div className="mb-6 flex flex-wrap items-center justify-between gap-3">
        <h1 className="text-2xl font-bold tracking-tight">
          <span className="font-mono font-bold text-primary">&gt;</span> 仮説
        </h1>
        <div className="flex items-center gap-2">
          <StrategyFilterSelect
            value={strategy_id}
            onChange={(v) => {
              void navigate({
                search: (prev) => ({ ...prev, strategy_id: v }),
              })
            }}
          />
          <button
            type="button"
            onClick={() => {
              setCreating(true)
            }}
            className="h-9 border border-border bg-surface-strong px-3 font-mono text-xs text-muted-foreground-strong hover:border-primary hover:text-primary"
          >
            + 新規作成
          </button>
        </div>
      </div>

      {isPending ? (
        <div className="space-y-2">
          <Skeleton className="h-14 w-full" />
          <Skeleton className="h-14 w-full" />
          <Skeleton className="h-14 w-full" />
        </div>
      ) : (
        <section className="border border-border bg-card">
          <div className="flex items-baseline justify-between border-b border-border px-3.5 py-2">
            <h2 className="font-mono text-xs font-bold uppercase tracking-wider text-foreground">
              一覧
            </h2>
            <span className="font-mono text-2xs text-muted-foreground">
              {hypotheses?.length ?? 0}
            </span>
          </div>
          {hypotheses == null || hypotheses.length === 0 ? (
            <div className="px-3.5 py-3 font-mono text-xs text-muted-foreground">
              —
            </div>
          ) : (
            <div>
              {hypotheses.map((h) => (
                <Link
                  key={h.hypothesis_id}
                  to="/hypotheses/$hypothesisId"
                  params={{ hypothesisId: h.hypothesis_id }}
                  className="flex flex-col gap-1 border-b border-border px-3.5 py-2.5 last:border-b-0 hover:bg-surface-strong"
                >
                  <span className="line-clamp-2 text-sm text-foreground">
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
        </section>
      )}

      <CreateHypothesisDialog
        initialStrategyId={strategy_id}
        open={creating}
        onOpenChange={setCreating}
      />
    </div>
  )
}
