import { createFileRoute } from '@tanstack/react-router'
import { useState } from 'react'

import { CreateHypothesisButton } from '#components/hypothesis-list/create-hypothesis-button'
import { HypothesisList } from '#components/hypothesis-list/hypothesis-list'
import { HypothesisListSkeleton } from '#components/hypothesis-list/hypothesis-list-skeleton'
import { StrategyFilterSelect } from '#components/strategy-filter-select'
import { CreateHypothesisDialog } from '#components/strategy-home/create-hypothesis-dialog'
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
          <CreateHypothesisButton
            onClick={() => {
              setCreating(true)
            }}
          />
        </div>
      </div>

      {isPending ? (
        <HypothesisListSkeleton />
      ) : (
        <HypothesisList
          hypotheses={(hypotheses ?? []).map((hypothesis) => ({
            hypothesisId: hypothesis.hypothesis_id,
            title: hypothesis.title,
            status: hypothesis.status,
            updatedAt: formatRelative(hypothesis.updated_at),
          }))}
        />
      )}

      <CreateHypothesisDialog
        initialStrategyId={strategy_id}
        open={creating}
        onOpenChange={setCreating}
      />
    </div>
  )
}
