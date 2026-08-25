import { useQueryClient } from '@tanstack/react-query'

import { StatusPill } from '#components/strategy-home/status-pill'
import { $api } from '#lib/api/client'

interface ReviewPanelProps {
  noteId: string
  strategyId: string
  status: string
}

export function ReviewPanel({ noteId, strategyId, status }: ReviewPanelProps) {
  const queryClient = useQueryClient()
  const approve = $api.useMutation('post', '/api/notes/{id}/approve')
  const reject = $api.useMutation('post', '/api/notes/{id}/reject')

  const invalidate = (): void => {
    void queryClient.invalidateQueries({
      queryKey: $api.queryOptions('get', '/api/notes/{id}', {
        params: { path: { id: noteId } },
      }).queryKey,
    })
    void queryClient.invalidateQueries({
      queryKey: $api.queryOptions('get', '/api/notes', {
        params: { query: { strategy_id: strategyId } },
      }).queryKey,
    })
    void queryClient.invalidateQueries({
      queryKey: $api.queryOptions('get', '/api/history', {
        params: { query: { target_kind: 'note', target_id: noteId } },
      }).queryKey,
    })
  }

  const pending = approve.isPending || reject.isPending

  const onApprove = (): void => {
    approve.mutate(
      { params: { path: { id: noteId } }, body: {} },
      { onSuccess: invalidate },
    )
  }
  const onReject = (): void => {
    reject.mutate(
      { params: { path: { id: noteId } }, body: {} },
      { onSuccess: invalidate },
    )
  }

  const label =
    status === 'approved'
      ? '承認済み'
      : status === 'rejected'
        ? '却下済み'
        : '未レビュー'

  return (
    <section className="border border-border bg-card">
      <header className="flex items-center justify-between border-b border-border px-3.5 py-2">
        <h3 className="font-mono text-xs font-bold uppercase tracking-wider text-foreground">
          レビュー
        </h3>
        <StatusPill status={status} />
      </header>
      <div className="space-y-3 px-3.5 py-3 font-mono text-xs">
        <div className="text-muted-foreground">
          現在: <strong className="text-foreground">{label}</strong>
        </div>
        <div className="flex flex-wrap gap-2">
          <button
            type="button"
            disabled={pending || status === 'approved'}
            onClick={onApprove}
            className={`inline-flex items-center gap-1 border px-2.5 py-1 text-xs ${
              status === 'approved'
                ? 'border-status-approved text-status-approved'
                : 'border-border text-muted-foreground-strong hover:border-status-approved hover:text-status-approved'
            } disabled:opacity-50`}
          >
            ✓ 承認
          </button>
          <button
            type="button"
            disabled={pending || status === 'rejected'}
            onClick={onReject}
            className={`inline-flex items-center gap-1 border px-2.5 py-1 text-xs ${
              status === 'rejected'
                ? 'border-primary text-primary'
                : 'border-border text-muted-foreground-strong hover:border-primary hover:text-primary'
            } disabled:opacity-50`}
          >
            ✕ 却下
          </button>
        </div>
        {(approve.isError || reject.isError) && (
          <p className="text-2xs text-primary">status の更新に失敗しました</p>
        )}
      </div>
    </section>
  )
}
