import { StatusPill } from '#components/strategy-home/status-pill'
import type { NoteVersion } from '#lib/api/note-version-types'

interface NoteVersionReviewViewProps {
  version: NoteVersion
  isDecisionInline: boolean
  lineCommentCount: number
  isCommentsPending: boolean
  reason: string
  recentlyReviewedStatus: 'approved' | 'rejected' | null
  isApproving: boolean
  isRejecting: boolean
  isMakingCurrent: boolean
  hasError: boolean
  onReasonChange: (reason: string) => void
  onApprove: () => void
  onReject: () => void
  onMakeCurrent: () => void
}

export function NoteVersionReviewView({
  version,
  isDecisionInline,
  lineCommentCount,
  isCommentsPending,
  reason,
  recentlyReviewedStatus,
  isApproving,
  isRejecting,
  isMakingCurrent,
  hasError,
  onReasonChange,
  onApprove,
  onReject,
  onMakeCurrent,
}: NoteVersionReviewViewProps) {
  const pending = isApproving || isRejecting || isMakingCurrent
  const needsReason = lineCommentCount === 0
  const canReject = !needsReason || reason.trim() !== ''
  const isReviewed =
    recentlyReviewedStatus != null ||
    version.status === 'approved' ||
    version.status === 'rejected' ||
    version.status === 'superseded'

  return (
    <section className="border border-border bg-card">
      <header className="flex items-center justify-between border-b border-border px-3.5 py-2">
        <h2 className="font-mono text-xs font-bold uppercase tracking-wider text-foreground">
          このバージョンを判断する
        </h2>
        <StatusPill status={version.status} />
      </header>
      <div className="space-y-3 px-3.5 py-3 font-mono text-xs">
        <div className="flex flex-wrap items-center justify-between gap-2 text-muted-foreground">
          <span>
            行コメント {String(lineCommentCount)} 件
            {isCommentsPending && ' · 読み込み中'}
          </span>
          {version.is_current && (
            <span className="border border-primary px-1.5 py-0.5 text-2xs text-primary">
              現行
            </span>
          )}
        </div>
        {recentlyReviewedStatus != null && (
          <p
            role="status"
            className={
              recentlyReviewedStatus === 'approved'
                ? 'text-status-approved'
                : 'text-status-rejected'
            }
          >
            {recentlyReviewedStatus === 'approved'
              ? '承認しました'
              : '却下しました'}
          </p>
        )}
        {!isReviewed && (
          <>
            <label className="block space-y-1 text-2xs text-muted-foreground-strong">
              <span>却下理由{needsReason ? ' (必須)' : ' (任意)'}</span>
              <textarea
                value={reason}
                onChange={(event) => {
                  onReasonChange(event.target.value)
                }}
                rows={2}
                className="w-full border border-border bg-background px-2 py-1.5 font-mono text-xs text-foreground outline-none focus:border-muted-foreground"
                placeholder={
                  needsReason
                    ? '行コメントがないため理由を入力してください'
                    : '必要であれば理由を入力してください'
                }
              />
            </label>
            <div
              className={`flex gap-2 ${isDecisionInline ? 'max-lg:fixed max-lg:inset-x-0 max-lg:bottom-0 max-lg:z-40 max-lg:justify-between max-lg:border-t max-lg:border-border max-lg:bg-card max-lg:px-4 max-lg:py-3 max-lg:shadow-lg' : ''}`}
            >
              <button
                type="button"
                disabled={pending || isCommentsPending || !canReject}
                onClick={onReject}
                className="flex-1 border border-border px-2.5 py-2 text-muted-foreground-strong hover:border-status-rejected hover:text-status-rejected disabled:opacity-50"
              >
                ✕ 却下
              </button>
              <button
                type="button"
                disabled={pending || isCommentsPending}
                onClick={onApprove}
                className="flex-1 border border-status-approved px-2.5 py-2 text-status-approved hover:bg-status-approved/10 disabled:opacity-50"
              >
                ✓ 承認
              </button>
            </div>
          </>
        )}
        {version.status === 'approved' && !version.is_current && (
          <button
            type="button"
            disabled={pending}
            onClick={onMakeCurrent}
            className="border border-border px-2.5 py-1 text-muted-foreground-strong hover:border-primary hover:text-primary disabled:opacity-50"
          >
            {isMakingCurrent
              ? '現行に戻しています…'
              : 'このバージョンを現行に戻す'}
          </button>
        )}
        {hasError && (
          <p className="text-2xs text-primary">
            レビュー状態を更新できませんでした
          </p>
        )}
      </div>
    </section>
  )
}
