import { useQueryClient } from '@tanstack/react-query'
import { Link } from '@tanstack/react-router'
import { useMemo, useState } from 'react'

import { HypothesisStatusPill } from '#components/strategy-home/hypothesis-status-pill'
import { Button } from '#components/ui/button'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from '#components/ui/dialog'
import { Input } from '#components/ui/input'
import { $api } from '#lib/api/client'
import type { components } from '#lib/api/schema.gen'

type NoteHypothesis = components['schemas']['NoteHypothesis']
type Hypothesis = components['schemas']['Hypothesis']

interface NoteHypothesesPanelProps {
  noteId: string
  strategyId: string | null
}

interface NoteHypothesesPanelViewProps {
  hypotheses: NoteHypothesis[]
  candidates: Hypothesis[]
  isPending: boolean
  isError: boolean
  isCandidatePending: boolean
  isCandidateError: boolean
  isDialogOpen: boolean
  isAttaching: boolean
  removingHypothesisId?: string
  query: string
  strategyAvailable: boolean
  hasMutationError: boolean
  onDialogOpenChange: (open: boolean) => void
  onQueryChange: (query: string) => void
  onAttach: (hypothesisId: string) => void
  onRemove: (hypothesisId: string) => void
}

export function NoteHypothesesPanel({
  noteId,
  strategyId,
}: NoteHypothesesPanelProps) {
  const queryClient = useQueryClient()
  const [isDialogOpen, setIsDialogOpen] = useState(false)
  const [query, setQuery] = useState('')

  const {
    data: hypotheses,
    isPending,
    isError,
  } = $api.useQuery('get', '/api/notes/{id}/hypotheses', {
    params: { path: { id: noteId } },
  })
  const {
    data: strategyHypotheses,
    isPending: isCandidatePending,
    isError: isCandidateError,
  } = $api.useQuery('get', '/api/hypotheses', {
    params: { query: { strategy_id: strategyId ?? undefined } },
    enabled: isDialogOpen && strategyId != null,
  })

  const linkedHypothesisIds = useMemo(
    () =>
      new Set((hypotheses ?? []).map((hypothesis) => hypothesis.hypothesis_id)),
    [hypotheses],
  )
  const candidates = useMemo(() => {
    const normalizedQuery = query.trim().toLocaleLowerCase()
    return (strategyHypotheses ?? []).filter((hypothesis) => {
      if (linkedHypothesisIds.has(hypothesis.hypothesis_id)) return false
      if (normalizedQuery === '') return true
      return `${hypothesis.title}\n${hypothesis.body}`
        .toLocaleLowerCase()
        .includes(normalizedQuery)
    })
  }, [linkedHypothesisIds, query, strategyHypotheses])

  const invalidateLinks = () =>
    queryClient.invalidateQueries({
      queryKey: $api.queryOptions('get', '/api/notes/{id}/hypotheses', {
        params: { path: { id: noteId } },
      }).queryKey,
    })
  const attachMutation = $api.useMutation('post', '/api/notes/{id}/hypotheses')
  const removeMutation = $api.useMutation(
    'delete',
    '/api/notes/{id}/hypotheses/{hypothesis_id}',
  )

  const attach = (hypothesisId: string) => {
    attachMutation.mutate(
      {
        params: { path: { id: noteId } },
        body: { hypothesis_id: hypothesisId },
      },
      {
        onSuccess: () => {
          void invalidateLinks()
          setIsDialogOpen(false)
          setQuery('')
        },
      },
    )
  }

  const remove = (hypothesisId: string) => {
    removeMutation.mutate(
      { params: { path: { id: noteId, hypothesis_id: hypothesisId } } },
      { onSuccess: () => void invalidateLinks() },
    )
  }

  return (
    <NoteHypothesesPanelView
      hypotheses={hypotheses ?? []}
      candidates={candidates}
      isPending={isPending}
      isError={isError}
      isCandidatePending={isCandidatePending}
      isCandidateError={isCandidateError}
      isDialogOpen={isDialogOpen}
      isAttaching={attachMutation.isPending}
      removingHypothesisId={
        removeMutation.isPending
          ? removeMutation.variables.params.path.hypothesis_id
          : undefined
      }
      query={query}
      strategyAvailable={strategyId != null}
      hasMutationError={attachMutation.isError || removeMutation.isError}
      onDialogOpenChange={(open) => {
        setIsDialogOpen(open)
        if (!open) setQuery('')
      }}
      onQueryChange={setQuery}
      onAttach={attach}
      onRemove={remove}
    />
  )
}

export function NoteHypothesesPanelView({
  hypotheses,
  candidates,
  isPending,
  isError,
  isCandidatePending,
  isCandidateError,
  isDialogOpen,
  isAttaching,
  removingHypothesisId,
  query,
  strategyAvailable,
  hasMutationError,
  onDialogOpenChange,
  onQueryChange,
  onAttach,
  onRemove,
}: NoteHypothesesPanelViewProps) {
  return (
    <section className="border border-border bg-card">
      <header className="flex items-center justify-between border-b border-border px-3.5 py-2">
        <h3 className="font-mono text-xs font-bold uppercase tracking-wider text-foreground">
          仮説
        </h3>
        {strategyAvailable && (
          <Button
            type="button"
            variant="outline"
            size="xs"
            onClick={() => {
              onDialogOpenChange(true)
            }}
          >
            + 紐付け
          </Button>
        )}
      </header>
      <div className="divide-y divide-border">
        <p className="px-3.5 py-2 font-mono text-2xs text-muted-foreground">
          タイトルと状態は紐付け時点のスナップショットです。
        </p>
        {isPending ? (
          <div className="px-3.5 py-3 font-mono text-xs text-muted-foreground">
            読み込み中…
          </div>
        ) : isError ? (
          <div className="px-3.5 py-3 font-mono text-xs text-primary">
            仮説を読み込めませんでした
          </div>
        ) : hypotheses.length === 0 ? (
          <div className="px-3.5 py-3 font-mono text-xs text-muted-foreground">
            {strategyAvailable
              ? '仮説はまだ紐付いていません'
              : '戦略に属さないノートには仮説を紐付けできません'}
          </div>
        ) : (
          hypotheses.map((hypothesis) => (
            <div
              key={hypothesis.hypothesis_id}
              className="space-y-2 px-3.5 py-3"
            >
              <div className="flex items-start justify-between gap-2">
                <LinkToHypothesis hypothesis={hypothesis} />
                <Button
                  type="button"
                  variant="ghost"
                  size="xs"
                  disabled={removingHypothesisId != null}
                  onClick={() => {
                    onRemove(hypothesis.hypothesis_id)
                  }}
                  aria-label={`「${hypothesis.hypothesis_title}」の紐付けを解除`}
                >
                  解除
                </Button>
              </div>
              <div className="flex items-center gap-2 font-mono text-2xs">
                <HypothesisStatusPill status={hypothesis.hypothesis_status} />
                <span className="text-muted-foreground">紐付け時点</span>
              </div>
            </div>
          ))
        )}
        {hasMutationError && (
          <p className="px-3.5 py-2 font-mono text-2xs text-primary">
            紐付けの更新に失敗しました
          </p>
        )}
      </div>

      <Dialog open={isDialogOpen} onOpenChange={onDialogOpenChange}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>仮説を紐付ける</DialogTitle>
            <DialogDescription>
              同じ戦略の仮説を選びます。現在のタイトルと状態が紐付け時点の記録として保存されます。
            </DialogDescription>
          </DialogHeader>
          <Input
            aria-label="仮説を絞り込む"
            value={query}
            onChange={(event) => {
              onQueryChange(event.target.value)
            }}
            placeholder="タイトルまたは本文で絞り込む"
          />
          <div className="max-h-80 divide-y divide-border overflow-y-auto border border-border">
            {isCandidatePending ? (
              <p className="px-3.5 py-3 font-mono text-xs text-muted-foreground">
                仮説を読み込み中…
              </p>
            ) : isCandidateError ? (
              <p className="px-3.5 py-3 font-mono text-xs text-primary">
                仮説を読み込めませんでした
              </p>
            ) : candidates.length === 0 ? (
              <p className="px-3.5 py-3 font-mono text-xs text-muted-foreground">
                {query.trim() === ''
                  ? '紐付け可能な仮説がありません'
                  : '条件に一致する仮説がありません'}
              </p>
            ) : (
              candidates.map((hypothesis) => (
                <button
                  type="button"
                  key={hypothesis.hypothesis_id}
                  disabled={isAttaching}
                  onClick={() => {
                    onAttach(hypothesis.hypothesis_id)
                  }}
                  className="flex w-full flex-col gap-1 px-3.5 py-3 text-left hover:bg-surface-strong disabled:opacity-50"
                >
                  <span className="text-sm text-foreground">
                    {hypothesis.title}
                  </span>
                  <span className="line-clamp-2 font-mono text-2xs text-muted-foreground">
                    {hypothesis.body}
                  </span>
                  <HypothesisStatusPill status={hypothesis.status} />
                </button>
              ))
            )}
          </div>
          {hasMutationError && (
            <p className="font-mono text-2xs text-primary">
              仮説を紐付けられませんでした
            </p>
          )}
        </DialogContent>
      </Dialog>
    </section>
  )
}

function LinkToHypothesis({ hypothesis }: { hypothesis: NoteHypothesis }) {
  return (
    <Link
      to="/hypotheses/$hypothesisId"
      params={{ hypothesisId: hypothesis.hypothesis_id }}
      className="line-clamp-2 text-sm text-foreground hover:text-primary"
      aria-label={`現在の仮説を開く: ${hypothesis.hypothesis_title}`}
    >
      {hypothesis.hypothesis_title}
    </Link>
  )
}
