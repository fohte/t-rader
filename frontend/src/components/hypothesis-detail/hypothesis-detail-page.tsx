import { Link } from '@tanstack/react-router'
import { useState } from 'react'

import { HypothesisEditor } from '@/components/hypothesis-detail/hypothesis-editor'
import { HypothesisStatusPanel } from '@/components/hypothesis-detail/hypothesis-status-panel'
import { useInvalidateHypothesis } from '@/components/hypothesis-detail/use-invalidate-hypothesis'
import { Skeleton } from '@/components/ui/skeleton'
import { $api } from '@/lib/api/client'

interface HypothesisDetailPageProps {
  strategyId: string
  hypothesisId: string
}

export function HypothesisDetailPage({
  strategyId,
  hypothesisId,
}: HypothesisDetailPageProps) {
  const {
    data: hypothesis,
    isPending,
    isError,
  } = $api.useQuery('get', '/api/strategies/{id}/hypotheses/{hypothesis_id}', {
    params: { path: { id: strategyId, hypothesis_id: hypothesisId } },
  })
  const invalidate = useInvalidateHypothesis(strategyId, hypothesisId)
  const updateMutation = $api.useMutation(
    'patch',
    '/api/strategies/{id}/hypotheses/{hypothesis_id}',
  )
  const [saveError, setSaveError] = useState<string | null>(null)

  function handleSave(next: { title: string; body: string }) {
    setSaveError(null)
    updateMutation.mutate(
      {
        params: { path: { id: strategyId, hypothesis_id: hypothesisId } },
        body: next,
      },
      {
        onSuccess: invalidate,
        onError: () => {
          setSaveError('保存に失敗しました')
        },
      },
    )
  }

  if (isPending) {
    return (
      <div className="space-y-4">
        <Skeleton className="h-6 w-32" />
        <Skeleton className="h-10 w-2/3" />
        <Skeleton className="h-[320px] w-full" />
      </div>
    )
  }

  if (isError) {
    return (
      <div
        data-testid="hypothesis-detail-error"
        className="font-mono text-[13px] text-[color:var(--color-accent-strategy)]"
      >
        仮説が見つからないか、取得に失敗しました。
      </div>
    )
  }

  return (
    <div className="space-y-4 font-sans text-[color:var(--color-text-primary)]">
      <Link
        to="/strategies/$id"
        params={{ id: strategyId }}
        className="inline-flex items-center gap-1 font-mono text-[12px] text-[color:var(--color-text-tertiary)] hover:text-[color:var(--color-accent-strategy)]"
      >
        &lt; 戦略ホームに戻る
      </Link>
      <div className="grid grid-cols-1 gap-5 lg:grid-cols-[minmax(0,1fr)_280px]">
        <article className="border border-[color:var(--color-border-strategy)] bg-[color:var(--panel)] px-5 py-5">
          <HypothesisEditor
            initialTitle={hypothesis.title}
            initialBody={hypothesis.body}
            onSave={handleSave}
            isSaving={updateMutation.isPending}
            saveError={saveError}
          />
        </article>
        <aside className="space-y-4">
          <HypothesisStatusPanel
            strategyId={strategyId}
            hypothesisId={hypothesisId}
            status={hypothesis.status}
          />
        </aside>
      </div>
    </div>
  )
}
