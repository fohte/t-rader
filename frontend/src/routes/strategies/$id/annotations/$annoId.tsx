import { useQueryClient } from '@tanstack/react-query'
import { createFileRoute, Link } from '@tanstack/react-router'
import { useState } from 'react'

import { StatusPill } from '#components/strategy-home/status-pill'
import { Skeleton } from '#components/ui/skeleton'
import { $api } from '#lib/api/client'
import { resolveRef } from '#lib/strategy-mock'

export const Route = createFileRoute('/strategies/$id/annotations/$annoId')({
  component: AnnotationDetailPage,
})

function formatDateTime(iso: string): string {
  const d = new Date(iso)
  if (Number.isNaN(d.getTime())) return iso
  const y = d.getFullYear()
  const m = String(d.getMonth() + 1).padStart(2, '0')
  const day = String(d.getDate()).padStart(2, '0')
  const h = String(d.getHours()).padStart(2, '0')
  const min = String(d.getMinutes()).padStart(2, '0')
  return `${String(y)}-${m}-${day} ${h}:${min}`
}

function AnnotationDetailPage() {
  const { id, annoId } = Route.useParams()
  const queryClient = useQueryClient()

  const { data: annotation, isPending: annoPending } = $api.useQuery(
    'get',
    '/api/annotations/{id}',
    { params: { path: { id: annoId } } },
  )

  const { data: comments } = $api.useQuery('get', '/api/comments', {
    params: { query: { target_kind: 'annotation', target_id: annoId } },
  })

  const { data: history } = $api.useQuery('get', '/api/history', {
    params: { query: { target_kind: 'annotation', target_id: annoId } },
  })

  const invalidate = () => {
    void queryClient.invalidateQueries({
      queryKey: $api.queryOptions('get', '/api/annotations/{id}', {
        params: { path: { id: annoId } },
      }).queryKey,
    })
    void queryClient.invalidateQueries({
      queryKey: $api.queryOptions('get', '/api/annotations').queryKey,
    })
    void queryClient.invalidateQueries({
      queryKey: $api.queryOptions('get', '/api/history').queryKey,
    })
  }

  const approveMutation = $api.useMutation(
    'post',
    '/api/annotations/{id}/approve',
    { onSuccess: invalidate },
  )
  const rejectMutation = $api.useMutation(
    'post',
    '/api/annotations/{id}/reject',
    { onSuccess: invalidate },
  )

  const createCommentMutation = $api.useMutation('post', '/api/comments', {
    onSuccess: () => {
      void queryClient.invalidateQueries({
        queryKey: $api.queryOptions('get', '/api/comments', {
          params: {
            query: { target_kind: 'annotation', target_id: annoId },
          },
        }).queryKey,
      })
    },
  })

  const [commentDraft, setCommentDraft] = useState('')

  if (annoPending) {
    return (
      <div className="space-y-4">
        <Skeleton className="h-8 w-72" />
        <Skeleton className="h-5 w-full max-w-[480px]" />
        <Skeleton className="h-[200px] w-full" />
      </div>
    )
  }

  if (annotation == null) {
    return (
      <div className="font-mono text-[13px] text-[color:var(--color-text-tertiary)]">
        アノテーションが見つかりませんでした。{' '}
        <Link
          to="/strategies/$id"
          params={{ id }}
          className="text-[color:var(--color-accent-strategy)] hover:underline"
        >
          戦略ホームに戻る
        </Link>
      </div>
    )
  }

  const stockRef = resolveRef(`stock:${annotation.target_symbol}`)

  function handleApprove() {
    if (approveMutation.isPending) return
    approveMutation.mutate({ params: { path: { id: annoId } }, body: {} })
  }

  function handleReject() {
    if (rejectMutation.isPending) return
    rejectMutation.mutate({ params: { path: { id: annoId } }, body: {} })
  }

  function handleAddComment(e: React.SyntheticEvent<HTMLFormElement>) {
    e.preventDefault()
    const body = commentDraft.trim()
    if (body === '' || createCommentMutation.isPending) return
    createCommentMutation.mutate(
      {
        body: {
          target_kind: 'annotation',
          target_id: annoId,
          body,
        },
      },
      {
        onSuccess: () => {
          setCommentDraft('')
        },
      },
    )
  }

  return (
    <div className="space-y-5 font-sans text-[color:var(--color-text-primary)]">
      <div>
        <Link
          to="/strategies/$id"
          params={{ id }}
          className="inline-flex items-center gap-1 font-mono text-[12px] text-[color:var(--color-text-secondary)] hover:text-[color:var(--color-accent-strategy)]"
        >
          ← 戦略ホームに戻る
        </Link>
      </div>

      <header className="border border-[color:var(--color-border-strategy)] bg-[color:var(--panel)] p-5">
        <div className="mb-3 flex flex-wrap items-center gap-2">
          <span className="inline-grid h-6 min-w-[32px] place-items-center border border-[color:var(--color-accent-strategy)] px-1 font-mono text-[11px] text-[color:var(--color-accent-strategy)]">
            ANNOTATION
          </span>
          <span className="border border-[color:var(--color-border-strategy)] bg-[color:var(--panel-inset)] px-1.5 py-px font-mono text-[10px] uppercase text-[color:var(--color-text-secondary)]">
            {annotation.target_kind}
          </span>
          <StatusPill status={annotation.status} />
        </div>
        <p className="mb-4 whitespace-pre-wrap text-[15px] leading-relaxed">
          {annotation.text}
        </p>
        <dl className="grid grid-cols-1 gap-y-1.5 font-mono text-[12px] text-[color:var(--color-text-secondary)] sm:grid-cols-[120px_minmax(0,1fr)]">
          <dt>銘柄</dt>
          <dd>
            {stockRef.name}
            <span className="ml-2 text-[color:var(--color-text-tertiary)]">
              {annotation.target_symbol}
            </span>
          </dd>
          <dt>timestamp</dt>
          <dd>{formatDateTime(annotation.timestamp)}</dd>
          {annotation.price != null && (
            <>
              <dt>price</dt>
              <dd>¥{annotation.price.toLocaleString()}</dd>
            </>
          )}
          <dt>作成者</dt>
          <dd>{annotation.created_by_kind}</dd>
          <dt>作成日時</dt>
          <dd>{formatDateTime(annotation.created_at)}</dd>
        </dl>
        {annotation.linked_note_id != null && (
          <div className="mt-4 border-t border-[color:var(--color-hairline)] pt-3">
            <Link
              to="/strategies/$id/notes/$noteId"
              params={{ id, noteId: annotation.linked_note_id }}
              className="inline-flex items-center gap-1 font-mono text-[12px] text-[color:var(--color-accent-strategy)] hover:underline"
            >
              → 紐づくノートを開く
            </Link>
          </div>
        )}
      </header>

      <section className="border border-[color:var(--color-border-strategy)] bg-[color:var(--panel)] p-5">
        <div className="mb-3 flex items-center justify-between">
          <h2 className="font-mono text-[11px] uppercase tracking-wider text-[color:var(--color-text-tertiary)]">
            レビュー
          </h2>
          <StatusPill status={annotation.status} />
        </div>
        <p className="mb-3 font-mono text-[12px] text-[color:var(--color-text-secondary)]">
          現在:{' '}
          <strong className="text-[color:var(--color-text-primary)]">
            {annotation.status === 'approved'
              ? '承認済み'
              : annotation.status === 'rejected'
                ? '却下'
                : '未レビュー'}
          </strong>
        </p>
        <div className="flex flex-wrap gap-2">
          <button
            type="button"
            onClick={handleApprove}
            disabled={
              approveMutation.isPending || annotation.status === 'approved'
            }
            className="cursor-pointer border border-[color:var(--color-accent-strategy)] bg-[color:var(--color-accent-strategy)] px-3 py-1.5 font-mono text-[12px] text-[color:var(--panel)] hover:opacity-90 disabled:cursor-not-allowed disabled:opacity-50"
          >
            ✓ 承認
          </button>
          <button
            type="button"
            onClick={handleReject}
            disabled={
              rejectMutation.isPending || annotation.status === 'rejected'
            }
            className="cursor-pointer border border-[color:var(--color-status-rejected)] px-3 py-1.5 font-mono text-[12px] text-[color:var(--color-status-rejected)] hover:bg-[color:var(--panel-inset)] disabled:cursor-not-allowed disabled:opacity-50"
          >
            ✕ 却下
          </button>
        </div>
        {(approveMutation.error != null || rejectMutation.error != null) && (
          <p className="mt-2 font-mono text-[11px] text-[color:var(--color-status-rejected)]">
            操作に失敗しました。時間を置いて再試行してください。
          </p>
        )}
      </section>

      <section className="border border-[color:var(--color-border-strategy)] bg-[color:var(--panel)] p-5">
        <h2 className="mb-3 font-mono text-[11px] uppercase tracking-wider text-[color:var(--color-text-tertiary)]">
          コメント · {comments?.length ?? 0}
        </h2>
        <div className="space-y-3">
          {(comments ?? []).map((c) => (
            <div
              key={c.id}
              className="border-l-2 border-[color:var(--color-border-strategy)] pl-3"
            >
              <div className="mb-1 flex items-center gap-2 font-mono text-[11px] text-[color:var(--color-text-tertiary)]">
                <span className="text-[color:var(--color-text-primary)]">
                  {c.author_label}
                </span>
                <span className="border border-[color:var(--color-border-strategy)] px-1 text-[9px] uppercase">
                  {c.author_kind}
                </span>
                <span>{formatDateTime(c.created_at)}</span>
              </div>
              <p className="whitespace-pre-wrap text-[13px] leading-relaxed">
                {c.body}
              </p>
            </div>
          ))}
          {(comments == null || comments.length === 0) && (
            <p className="font-mono text-[12px] text-[color:var(--color-text-tertiary)]">
              まだコメントはありません。
            </p>
          )}
        </div>
        <form onSubmit={handleAddComment} className="mt-4 flex gap-2">
          <input
            type="text"
            value={commentDraft}
            onChange={(e) => {
              setCommentDraft(e.target.value)
            }}
            placeholder="コメントを追加"
            className="flex-1 border border-[color:var(--color-border-strategy)] bg-[color:var(--panel-inset)] px-2 py-1.5 font-mono text-[12px] outline-none focus:border-[color:var(--color-accent-strategy)]"
          />
          <button
            type="submit"
            disabled={
              commentDraft.trim() === '' || createCommentMutation.isPending
            }
            className="cursor-pointer border border-[color:var(--color-border-strategy)] bg-[color:var(--panel-inset)] px-3 py-1.5 font-mono text-[12px] hover:border-[color:var(--color-accent-strategy)] disabled:cursor-not-allowed disabled:opacity-50"
          >
            送信
          </button>
        </form>
        {createCommentMutation.error != null && (
          <p className="mt-2 font-mono text-[11px] text-[color:var(--color-status-rejected)]">
            送信に失敗しました。時間を置いて再試行してください。
          </p>
        )}
      </section>

      <section className="border border-[color:var(--color-border-strategy)] bg-[color:var(--panel)] p-5">
        <h2 className="mb-3 font-mono text-[11px] uppercase tracking-wider text-[color:var(--color-text-tertiary)]">
          変更履歴 · {history?.length ?? 0}
        </h2>
        <div className="space-y-2 font-mono text-[12px]">
          {(history ?? []).map((h) => (
            <div
              key={h.id}
              className="flex flex-wrap items-baseline gap-2 border-b border-[color:var(--color-hairline)] pb-1.5 last:border-b-0"
            >
              <span className="text-[color:var(--color-text-tertiary)]">
                {formatDateTime(h.created_at)}
              </span>
              <span
                className={
                  h.actor_kind === 'human'
                    ? 'text-[color:var(--color-text-primary)]'
                    : 'text-[color:var(--color-text-secondary)]'
                }
              >
                {h.actor_label}
              </span>
              <span className="border border-[color:var(--color-border-strategy)] px-1 text-[10px] uppercase text-[color:var(--color-text-secondary)]">
                {h.op}
              </span>
              {h.summary != null && h.summary !== '' && (
                <span className="text-[color:var(--color-text-secondary)]">
                  {h.summary}
                </span>
              )}
            </div>
          ))}
          {(history == null || history.length === 0) && (
            <p className="text-[color:var(--color-text-tertiary)]">
              変更履歴はありません。
            </p>
          )}
        </div>
      </section>
    </div>
  )
}
