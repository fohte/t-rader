import { useQueryClient } from '@tanstack/react-query'

import { StatusPill } from '@/components/strategy-home/status-pill'
import { $api } from '@/lib/api/client'

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
    <section className="border border-[color:var(--color-border-strategy)] bg-[color:var(--panel)]">
      <header className="flex items-center justify-between border-b border-[color:var(--color-hairline)] px-3.5 py-2">
        <h3 className="font-mono text-[12px] font-bold uppercase tracking-wider text-[color:var(--color-text-primary)]">
          レビュー
        </h3>
        <StatusPill status={status} />
      </header>
      <div className="space-y-3 px-3.5 py-3 font-mono text-[12px]">
        <div className="text-[color:var(--color-text-tertiary)]">
          現在:{' '}
          <strong className="text-[color:var(--color-text-primary)]">
            {label}
          </strong>
        </div>
        <div className="flex flex-wrap gap-2">
          <button
            type="button"
            disabled={pending || status === 'approved'}
            onClick={onApprove}
            className={`inline-flex items-center gap-1 border px-2.5 py-1 text-[12px] ${
              status === 'approved'
                ? 'border-[color:var(--color-status-approved)] text-[color:var(--color-status-approved)]'
                : 'border-[color:var(--color-border-strategy)] text-[color:var(--color-text-secondary)] hover:border-[color:var(--color-status-approved)] hover:text-[color:var(--color-status-approved)]'
            } disabled:opacity-50`}
          >
            ✓ 承認
          </button>
          <button
            type="button"
            disabled={pending || status === 'rejected'}
            onClick={onReject}
            className={`inline-flex items-center gap-1 border px-2.5 py-1 text-[12px] ${
              status === 'rejected'
                ? 'border-[color:var(--color-accent-strategy)] text-[color:var(--color-accent-strategy)]'
                : 'border-[color:var(--color-border-strategy)] text-[color:var(--color-text-secondary)] hover:border-[color:var(--color-accent-strategy)] hover:text-[color:var(--color-accent-strategy)]'
            } disabled:opacity-50`}
          >
            ✕ 却下
          </button>
        </div>
        {(approve.isError || reject.isError) && (
          <p className="text-[11px] text-[color:var(--color-accent-strategy)]">
            status の更新に失敗しました
          </p>
        )}
      </div>
    </section>
  )
}
