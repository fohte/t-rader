import { StatusPill } from '#components/strategy-home/status-pill'
import type { NoteVersion } from '#lib/api/note-version-types'

interface NoteVersionReviewViewProps {
  version: NoteVersion
  lineCommentCount: number
  isCommentsPending: boolean
  reason: string
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
  lineCommentCount,
  isCommentsPending,
  reason,
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
    version.status === 'approved' ||
    version.status === 'rejected' ||
    version.status === 'superseded'

  return (
    <section className="border border-border bg-card">
      <header className="flex items-center justify-between border-b border-border px-3.5 py-2">
        <h2 className="font-mono text-xs font-bold uppercase tracking-wider text-foreground">
          バージョンレビュー
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
        {!isReviewed && (
          <>
            <div className="flex flex-wrap gap-2">
              <button
                type="button"
                disabled={pending || isCommentsPending}
                onClick={onApprove}
                className="border border-border px-2.5 py-1 text-muted-foreground-strong hover:border-status-approved hover:text-status-approved disabled:opacity-50"
              >
                ✓ 承認
              </button>
              <button
                type="button"
                disabled={pending || isCommentsPending || !canReject}
                onClick={onReject}
                className="border border-border px-2.5 py-1 text-muted-foreground-strong hover:border-status-rejected hover:text-status-rejected disabled:opacity-50"
              >
                ✕ 却下
              </button>
            </div>
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
