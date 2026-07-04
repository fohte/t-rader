import { useQueryClient } from '@tanstack/react-query'

import {
  HYPOTHESIS_STATUS_LABEL,
  HYPOTHESIS_STATUSES,
  HypothesisStatusPill,
} from '@/components/strategy-home/hypothesis-status-pill'
import { $api } from '@/lib/api/client'

interface HypothesisStatusPanelProps {
  strategyId: string
  hypothesisId: string
  status: string
}

export function HypothesisStatusPanel({
  strategyId,
  hypothesisId,
  status,
}: HypothesisStatusPanelProps) {
  const queryClient = useQueryClient()
  const updateMutation = $api.useMutation(
    'patch',
    '/api/strategies/{id}/hypotheses/{hypothesis_id}',
  )

  function handleChange(nextStatus: string) {
    updateMutation.mutate(
      {
        params: { path: { id: strategyId, hypothesis_id: hypothesisId } },
        body: { status: nextStatus },
      },
      {
        onSuccess: () => {
          void queryClient.invalidateQueries({
            queryKey: $api.queryOptions(
              'get',
              '/api/strategies/{id}/hypotheses/{hypothesis_id}',
              {
                params: {
                  path: { id: strategyId, hypothesis_id: hypothesisId },
                },
              },
            ).queryKey,
          })
          void queryClient.invalidateQueries({
            queryKey: $api.queryOptions(
              'get',
              '/api/strategies/{id}/hypotheses',
              { params: { path: { id: strategyId } } },
            ).queryKey,
          })
        },
      },
    )
  }

  return (
    <section className="border border-[color:var(--color-border-strategy)] bg-[color:var(--panel)]">
      <header className="flex items-center justify-between border-b border-[color:var(--color-hairline)] px-3.5 py-2">
        <h3 className="font-mono text-[12px] font-bold uppercase tracking-wider text-[color:var(--color-text-primary)]">
          status
        </h3>
        <HypothesisStatusPill status={status} />
      </header>
      <div className="px-3.5 py-3">
        <select
          aria-label="status"
          value={status}
          disabled={updateMutation.isPending}
          onChange={(e) => {
            handleChange(e.target.value)
          }}
          className="h-9 w-full rounded-md border border-input bg-transparent px-3 font-mono text-[12px]"
        >
          {HYPOTHESIS_STATUSES.map((s) => (
            <option key={s} value={s}>
              {HYPOTHESIS_STATUS_LABEL[s]}
            </option>
          ))}
        </select>
        {updateMutation.isError && (
          <p className="mt-2 font-mono text-[11px] text-[color:var(--color-accent-strategy)]">
            status の更新に失敗しました
          </p>
        )}
      </div>
    </section>
  )
}
