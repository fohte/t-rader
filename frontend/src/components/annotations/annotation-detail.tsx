import { Link } from '@tanstack/react-router'
import type { SyntheticEvent } from 'react'

import { DetailBackLink } from '#components/detail-back-link'
import { ReviewNavigationBand } from '#components/review/review-navigation-band'
import {
  type ReviewNavigationSnapshot,
  type ReviewQueueItem,
} from '#components/review/review-queue-model'
import { StockNameLink } from '#components/stocks/stock-name-link'
import { StatusPill } from '#components/strategy-home/status-pill'
import { Skeleton } from '#components/ui/skeleton'
import type { components } from '#lib/api/schema.gen'

type Annotation = components['schemas']['Annotation']
type Comment = components['schemas']['Comment']
type ChangeHistory = components['schemas']['ChangeHistory']

type AnnotationDetailProps = {
  annotation: Annotation | undefined
  isPending: boolean
  reviewNavigation: {
    items: ReviewQueueItem[]
    snapshot: ReviewNavigationSnapshot
    currentItemKey: string
  } | null
  reviewNavigationError: boolean
  comments: Comment[] | undefined
  history: ChangeHistory[] | undefined
  stockName: string | null
  commentDraft: string
  rejectionReason: string
  isApprovalPending: boolean
  isRejectionPending: boolean
  isCommentPending: boolean
  approvalError: unknown
  rejectionError: unknown
  commentError: unknown
  onApprove: () => void
  onReject: () => void
  onRejectionReasonChange: (value: string) => void
  onAddComment: (event: SyntheticEvent<HTMLFormElement>) => void
  onCommentDraftChange: (value: string) => void
}

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

export function AnnotationDetail({
  annotation,
  isPending,
  reviewNavigation,
  reviewNavigationError,
  comments,
  history,
  stockName,
  commentDraft,
  rejectionReason,
  isApprovalPending,
  isRejectionPending,
  isCommentPending,
  approvalError,
  rejectionError,
  commentError,
  onApprove,
  onReject,
  onRejectionReasonChange,
  onAddComment,
  onCommentDraftChange,
}: AnnotationDetailProps) {
  return (
    <div className="space-y-5 font-sans text-foreground">
      <DetailBackLink tab="annotations" label="アノテーション一覧に戻る" />
      {reviewNavigation != null && (
        <ReviewNavigationBand {...reviewNavigation} />
      )}
      {reviewNavigationError && (
        <p role="alert" className="font-mono text-xs text-status-rejected">
          未読の移動情報を読み込めませんでした。
        </p>
      )}
      {isPending || annotation == null ? (
        isPending ? (
          <div className="space-y-4">
            <Skeleton className="h-8 w-72" />
            <Skeleton className="h-5 w-full max-w-120" />
            <Skeleton className="h-50 w-full" />
          </div>
        ) : (
          <div className="font-mono text-sm text-muted-foreground">
            アノテーションが見つかりませんでした。
          </div>
        )
      ) : (
        <>
          <header className="border border-border bg-card p-5">
            <div className="mb-3 flex flex-wrap items-center gap-2">
              <span className="inline-grid h-6 min-w-8 place-items-center border border-primary px-1 font-mono text-2xs text-primary">
                ANNOTATION
              </span>
              <span className="border border-border bg-surface-strong px-1.5 py-px font-mono text-2xs uppercase text-muted-foreground-strong">
                {annotation.target_kind}
              </span>
              <StatusPill status={annotation.status} />
            </div>
            <p className="mb-4 whitespace-pre-wrap text-base leading-relaxed">
              {annotation.text}
            </p>
          </header>

          <section className="border border-border bg-card p-5">
            <h2 className="mb-3 font-mono text-2xs uppercase tracking-wider text-muted-foreground">
              このアノテーションを判断する
            </h2>
            {annotation.status === 'unread' ? (
              <>
                <div className="mb-3 flex flex-wrap gap-2">
                  <button
                    type="button"
                    onClick={onApprove}
                    disabled={isApprovalPending || isRejectionPending}
                    className="cursor-pointer border border-primary bg-primary px-3 py-1.5 font-mono text-xs text-card hover:opacity-90 disabled:cursor-not-allowed disabled:opacity-50"
                  >
                    ✓ 承認
                  </button>
                  <button
                    type="button"
                    onClick={onReject}
                    disabled={isApprovalPending || isRejectionPending}
                    className="cursor-pointer border border-status-rejected px-3 py-1.5 font-mono text-xs text-status-rejected hover:bg-surface-strong disabled:cursor-not-allowed disabled:opacity-50"
                  >
                    ✕ 却下
                  </button>
                </div>
                <label className="block space-y-1 font-mono text-2xs text-muted-foreground-strong">
                  <span>却下理由 (任意)</span>
                  <textarea
                    value={rejectionReason}
                    onChange={(event) => {
                      onRejectionReasonChange(event.target.value)
                    }}
                    disabled={isApprovalPending || isRejectionPending}
                    rows={2}
                    className="w-full border border-border bg-surface-strong px-2 py-1.5 font-mono text-xs text-foreground outline-none focus:border-primary disabled:opacity-50"
                    placeholder="必要であれば理由を入力してください"
                  />
                </label>
              </>
            ) : (
              <p className="font-mono text-xs text-muted-foreground-strong">
                {annotation.status === 'approved' ? '承認済み' : '却下済み'}
              </p>
            )}
            {(approvalError != null || rejectionError != null) && (
              <p className="mt-2 font-mono text-2xs text-status-rejected">
                操作に失敗しました。時間を置いて再試行してください。
              </p>
            )}
          </section>

          <section className="border border-border bg-card p-5">
            <dl className="grid grid-cols-1 gap-y-1.5 font-mono text-xs text-muted-foreground-strong sm:grid-cols-(--grid-cols-annotation-fields)">
              <dt>銘柄</dt>
              <dd>
                <StockNameLink
                  name={stockName}
                  symbol={annotation.target_symbol}
                  missingName="unresolved"
                />
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
              <div className="mt-4 border-t border-border pt-3">
                <Link
                  to="/notes/$noteId"
                  params={{ noteId: annotation.linked_note_id }}
                  className="inline-flex items-center gap-1 font-mono text-xs text-primary hover:underline"
                >
                  → 紐づくノートを開く
                </Link>
              </div>
            )}
          </section>

          <section className="border border-border bg-card p-5">
            <h2 className="mb-3 font-mono text-2xs uppercase tracking-wider text-muted-foreground">
              コメント · {comments?.length ?? 0}
            </h2>
            <div className="space-y-3">
              {(comments ?? []).map((comment) => (
                <div key={comment.id} className="border-l-2 border-border pl-3">
                  <div className="mb-1 flex items-center gap-2 font-mono text-2xs text-muted-foreground">
                    <span className="text-foreground">
                      {comment.author_label}
                    </span>
                    <span className="border border-border px-1 text-2xs uppercase">
                      {comment.author_kind}
                    </span>
                    <span>{formatDateTime(comment.created_at)}</span>
                  </div>
                  <p className="whitespace-pre-wrap text-sm leading-relaxed">
                    {comment.body}
                  </p>
                </div>
              ))}
              {(comments == null || comments.length === 0) && (
                <p className="font-mono text-xs text-muted-foreground">
                  まだコメントはありません。
                </p>
              )}
            </div>
            <form onSubmit={onAddComment} className="mt-4 flex gap-2">
              <input
                type="text"
                value={commentDraft}
                onChange={(event) => {
                  onCommentDraftChange(event.target.value)
                }}
                placeholder="コメントを追加"
                className="flex-1 border border-border bg-surface-strong px-2 py-1.5 font-mono text-xs outline-none focus:border-primary"
              />
              <button
                type="submit"
                disabled={commentDraft.trim() === '' || isCommentPending}
                className="cursor-pointer border border-border bg-surface-strong px-3 py-1.5 font-mono text-xs hover:border-primary disabled:cursor-not-allowed disabled:opacity-50"
              >
                送信
              </button>
            </form>
            {commentError != null && (
              <p className="mt-2 font-mono text-2xs text-status-rejected">
                送信に失敗しました。時間を置いて再試行してください。
              </p>
            )}
          </section>

          <section className="border border-border bg-card p-5">
            <h2 className="mb-3 font-mono text-2xs uppercase tracking-wider text-muted-foreground">
              変更履歴 · {history?.length ?? 0}
            </h2>
            <div className="space-y-2 font-mono text-xs">
              {(history ?? []).map((entry) => (
                <div
                  key={entry.id}
                  className="flex flex-wrap items-baseline gap-2 border-b border-border pb-1.5 last:border-b-0"
                >
                  <span className="text-muted-foreground">
                    {formatDateTime(entry.created_at)}
                  </span>
                  <span
                    className={
                      entry.actor_kind === 'human'
                        ? 'text-foreground'
                        : 'text-muted-foreground-strong'
                    }
                  >
                    {entry.actor_label}
                  </span>
                  <span className="border border-border px-1 text-2xs uppercase text-muted-foreground-strong">
                    {entry.op}
                  </span>
                  {entry.summary != null && entry.summary !== '' && (
                    <span className="text-muted-foreground-strong">
                      {entry.summary}
                    </span>
                  )}
                </div>
              ))}
              {(history == null || history.length === 0) && (
                <p className="text-muted-foreground">変更履歴はありません。</p>
              )}
            </div>
          </section>
        </>
      )}
    </div>
  )
}
