import { createFileRoute, Link } from '@tanstack/react-router'

import { StrategyFilterSelect } from '#components/strategy-filter-select'
import { StatusPill } from '#components/strategy-home/status-pill'
import { Skeleton } from '#components/ui/skeleton'
import { $api } from '#lib/api/client'
import { buildSnippet, formatRelative } from '#lib/note-utils'

export const Route = createFileRoute('/annotations/')({
  validateSearch: (
    search: Record<string, unknown>,
  ): { strategy_id?: string } => ({
    strategy_id:
      typeof search.strategy_id === 'string' ? search.strategy_id : undefined,
  }),
  component: AnnotationsPage,
})

function AnnotationsPage() {
  const { strategy_id } = Route.useSearch()
  const navigate = Route.useNavigate()

  const { data: annotations, isPending } = $api.useQuery(
    'get',
    '/api/annotations',
    { params: { query: { strategy_id } } },
  )

  return (
    <div className="font-sans text-foreground">
      <div className="mb-6 flex flex-wrap items-center justify-between gap-3">
        <h1 className="text-2xl font-bold tracking-tight">
          <span className="font-mono font-bold text-primary">&gt;</span>{' '}
          アノテーション
        </h1>
        <StrategyFilterSelect
          value={strategy_id}
          onChange={(v) => {
            void navigate({ search: (prev) => ({ ...prev, strategy_id: v }) })
          }}
        />
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
              {annotations?.length ?? 0}
            </span>
          </div>
          {annotations == null || annotations.length === 0 ? (
            <div className="px-3.5 py-3 font-mono text-xs text-muted-foreground">
              —
            </div>
          ) : (
            <div>
              {annotations.map((a) => (
                <Link
                  key={a.id}
                  to="/annotations/$annoId"
                  params={{ annoId: a.id }}
                  className="flex flex-col gap-1 border-b border-border px-3.5 py-2.5 last:border-b-0 hover:bg-surface-strong"
                >
                  <span className="line-clamp-2 text-sm text-foreground">
                    {buildSnippet(a.text)}
                  </span>
                  <span className="flex flex-wrap items-center gap-2 font-mono text-2xs">
                    <span className="border border-border bg-surface-strong px-1 text-2xs uppercase text-muted-foreground-strong">
                      {a.target_symbol}
                    </span>
                    <StatusPill status={a.status} />
                    <span className="ml-auto text-muted-foreground">
                      {formatRelative(a.updated_at)}
                    </span>
                  </span>
                </Link>
              ))}
            </div>
          )}
        </section>
      )}
    </div>
  )
}
