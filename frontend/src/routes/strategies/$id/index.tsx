import { createFileRoute, Link } from '@tanstack/react-router'
import { useMemo, useState } from 'react'

import { AnalysisCard } from '#components/strategy-home/analysis-card'
import { AnnotationList } from '#components/strategy-home/annotation-list'
import { ArrivalsList } from '#components/strategy-home/arrivals-list'
import { ChartPanel } from '#components/strategy-home/chart-panel'
import { HypothesisList } from '#components/strategy-home/hypothesis-list'
import { InterestTree } from '#components/strategy-home/interest-tree'
import { NotesList } from '#components/strategy-home/notes-list'
import { RelatedMacro } from '#components/strategy-home/related-macro'
import { RelatedNews } from '#components/strategy-home/related-news'
import { useLastVisited } from '#components/strategy-home/use-last-visited'
import { RefChip } from '#components/strategy-shell/ref-chip'
import { Skeleton } from '#components/ui/skeleton'
import { buildNumberedAnnotations } from '#lib/annotation-utils'
import { $api } from '#lib/api/client'
import { resolveRef } from '#lib/strategy-mock'

export const Route = createFileRoute('/strategies/$id/')({
  component: StrategyHomePage,
})

function StrategyHomePage() {
  const { id } = Route.useParams()
  const lastVisited = useLastVisited(id)
  const [activeSymbol, setActiveSymbol] = useState<string | null>(null)
  const [selectedAnnotationId, setSelectedAnnotationId] = useState<
    string | null
  >(null)

  const { data: strategy, isPending: strategyPending } = $api.useQuery(
    'get',
    '/api/strategies/{id}',
    { params: { path: { id } } },
  )
  const { data: interests } = $api.useQuery(
    'get',
    '/api/strategies/{id}/interests',
    { params: { path: { id } } },
  )
  const { data: notes } = $api.useQuery('get', '/api/notes', {
    params: { query: { strategy_id: id } },
  })
  const { data: annotations } = $api.useQuery('get', '/api/annotations', {
    params: { query: { strategy_id: id } },
  })

  const seeds = useMemo(
    () => interests?.filter((i) => i.role === 'seed') ?? [],
    [interests],
  )
  const stockSymbols = useMemo(
    () =>
      seeds
        .filter((i) => i.ref_kind === 'stock')
        .map((i) => ({
          code: i.ref_id,
          name: resolveRef(`stock:${i.ref_id}`).name,
        })),
    [seeds],
  )
  const indicatorIds = useMemo(
    () => seeds.filter((i) => i.ref_kind === 'indicator').map((i) => i.ref_id),
    [seeds],
  )

  const numberedAnnotations = useMemo(
    () => buildNumberedAnnotations(annotations ?? [], activeSymbol),
    [annotations, activeSymbol],
  )

  const sortedNotes = useMemo(
    () =>
      [...(notes ?? [])].sort((a, b) =>
        b.updated_at.localeCompare(a.updated_at),
      ),
    [notes],
  )

  if (strategyPending) {
    return (
      <div className="space-y-4">
        <Skeleton className="h-8 w-72" />
        <Skeleton className="h-5 w-full max-w-120" />
        <Skeleton className="h-100 w-full" />
      </div>
    )
  }

  if (strategy == null) {
    return (
      <div className="font-mono text-sm text-muted-foreground">
        戦略が見つかりませんでした。
      </div>
    )
  }

  return (
    <div className="space-y-6 font-sans text-foreground">
      <header className="flex flex-col gap-3 md:flex-row md:items-start md:justify-between">
        <div className="min-w-0 flex-1">
          <h1 className="mb-1.5 text-2xl font-bold leading-tight tracking-tight">
            {strategy.name}
          </h1>
          {strategy.description != null && strategy.description !== '' && (
            <p className="max-w-180 text-sm leading-relaxed text-muted-foreground-strong">
              {strategy.description}
            </p>
          )}
          {seeds.length > 0 && (
            <div className="mt-3 flex flex-wrap items-center gap-1.5">
              {seeds.map((s) => (
                <RefChip
                  key={`${s.ref_kind}:${s.ref_id}`}
                  token={`${s.ref_kind}:${s.ref_id}`}
                  pill
                />
              ))}
            </div>
          )}
        </div>
        <Link
          to="/strategies/$id/performance"
          params={{ id }}
          className="inline-flex flex-shrink-0 items-center gap-1.5 self-start border border-border bg-surface-strong px-3 py-1.5 font-mono text-xs text-muted-foreground-strong hover:border-primary hover:text-primary"
        >
          戦略成績 →
        </Link>
      </header>

      <ArrivalsList
        strategyId={id}
        notes={notes ?? []}
        annotations={annotations ?? []}
        since={lastVisited}
      />

      <div className="grid grid-cols-1 gap-5 lg:grid-cols-[minmax(0,1fr)_320px]">
        <div className="space-y-5">
          <ChartPanel
            symbols={stockSymbols}
            numberedAnnotations={numberedAnnotations}
            selectedAnnotationId={selectedAnnotationId}
            onSelectAnnotation={setSelectedAnnotationId}
            onSymbolChange={setActiveSymbol}
          />

          <AnnotationList
            strategyId={id}
            items={numberedAnnotations}
            symbol={activeSymbol}
            selectedAnnotationId={selectedAnnotationId}
            onSelectAnnotation={setSelectedAnnotationId}
          />

          <section>
            <div className="mb-2 flex items-baseline gap-2 font-mono text-2xs uppercase tracking-wider text-muted-foreground">
              <span className="text-primary">&gt;</span>
              <span>分析カードフィード</span>
              <span className="text-muted-foreground-strong">
                LLM 産出物 · 新しい順
              </span>
            </div>
            {sortedNotes.length === 0 ? (
              <div className="border border-border bg-card px-4 py-6 text-center font-mono text-xs text-muted-foreground">
                まだ産出物がありません。右下の{' '}
                <span className="text-primary">&gt;_</span> から on-demand
                セッションを起動してください。
              </div>
            ) : (
              <div className="space-y-3">
                {sortedNotes.map((n) => (
                  <AnalysisCard key={n.id} note={n} strategyId={id} />
                ))}
              </div>
            )}
          </section>
        </div>

        <aside className="space-y-5">
          <RelatedMacro indicatorIds={indicatorIds} />
          <RelatedNews strategyId={id} />
          <NotesList strategyId={id} notes={sortedNotes} />
          <HypothesisList strategyId={id} />
          <InterestTree strategyId={id} />
        </aside>
      </div>
    </div>
  )
}
