import { Button } from '@fohte/ui/button'
import { Chip } from '@fohte/ui/chip'
import { Textarea } from '@fohte/ui/textarea'

import { WarningDispositionReview } from '#components/note-detail/warning-disposition-review'
import { StatusPill } from '#components/strategy-home/status-pill'
import type { NoteVersion } from '#lib/api/note-version-types'
import { cn } from '#lib/utils'

export type NoteVersionReviewResult = 'approved' | 'rejected'

interface NoteVersionReviewViewProps {
  version: NoteVersion
  pinDecisionActionsOnMobile: boolean
  lineCommentCount: number
  isCommentsPending: boolean
  reason: string
  recentlyReviewedStatus: NoteVersionReviewResult | null
  isApproving: boolean
  isRejecting: boolean
  isMakingCurrent: boolean
  hasError: boolean
  warningDispositionsConfirmed: boolean
  onReasonChange: (reason: string) => void
  onWarningDispositionsConfirmedChange: (confirmed: boolean) => void
  onApprove: () => void
  onReject: () => void
  onMakeCurrent: () => void
}

export function NoteVersionReviewView({
  version,
  pinDecisionActionsOnMobile,
  lineCommentCount,
  isCommentsPending,
  reason,
  recentlyReviewedStatus,
  isApproving,
  isRejecting,
  isMakingCurrent,
  hasError,
  warningDispositionsConfirmed,
  onReasonChange,
  onWarningDispositionsConfirmedChange,
  onApprove,
  onReject,
  onMakeCurrent,
}: NoteVersionReviewViewProps) {
  const pending = isApproving || isRejecting || isMakingCurrent
  const needsReason = lineCommentCount === 0
  const canReject = !needsReason || reason.trim() !== ''
  const needsWarningConfirmation = version.warning_dispositions.length > 0
  const isReviewed =
    recentlyReviewedStatus != null ||
    version.status === 'approved' ||
    version.status === 'rejected' ||
    version.status === 'superseded'

  return (
    <section className="border border-border bg-card">
      <header className="flex items-center justify-between border-b border-border px-3.5 py-2">
        <h2 className="font-mono text-xs font-bold uppercase tracking-wider text-foreground">
          {isReviewed ? 'バージョンレビュー' : 'このバージョンを判断する'}
        </h2>
        <StatusPill status={version.status} />
      </header>
      <div className="space-y-3 px-3.5 py-3 font-mono text-xs">
        <div className="flex flex-wrap items-center justify-between gap-2 text-muted-foreground">
          <span>
            行コメント {String(lineCommentCount)} 件
            {isCommentsPending && ' · 読み込み中'}
          </span>
          {version.is_current && <Chip>現行</Chip>}
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
        <WarningDispositionReview
          versionNo={version.version_no}
          warningDispositions={version.warning_dispositions}
          confirmation={
            !isReviewed && needsWarningConfirmation
              ? {
                  checked: warningDispositionsConfirmed,
                  onChange: onWarningDispositionsConfirmedChange,
                }
              : undefined
          }
        />
        {!isReviewed && (
          <>
            <label className="block space-y-1 text-2xs text-muted-foreground-strong">
              <span>却下理由{needsReason ? ' (必須)' : ' (任意)'}</span>
              <Textarea
                value={reason}
                onChange={(event) => {
                  onReasonChange(event.target.value)
                }}
                placeholder={
                  needsReason
                    ? '行コメントがないため理由を入力してください'
                    : '必要であれば理由を入力してください'
                }
              />
            </label>
            <div
              className={cn(
                'flex gap-2',
                pinDecisionActionsOnMobile &&
                  'max-lg:fixed max-lg:inset-x-0 max-lg:bottom-0 max-lg:z-40 max-lg:justify-between max-lg:border-t max-lg:border-border max-lg:bg-card max-lg:px-4 max-lg:py-3 max-lg:shadow-lg',
              )}
            >
              <Button
                type="button"
                variant="destructive"
                size="lg"
                disabled={pending || isCommentsPending || !canReject}
                onClick={onReject}
                className="flex-1"
              >
                {isRejecting ? '却下中…' : '✕ 却下'}
              </Button>
              <Button
                type="button"
                size="lg"
                disabled={
                  pending ||
                  isCommentsPending ||
                  (needsWarningConfirmation && !warningDispositionsConfirmed)
                }
                onClick={onApprove}
                className="flex-1"
              >
                {isApproving ? '承認中…' : '✓ 承認'}
              </Button>
            </div>
          </>
        )}
        {version.status === 'approved' && !version.is_current && (
          <Button
            type="button"
            variant="outline"
            size="sm"
            disabled={pending}
            onClick={onMakeCurrent}
          >
            {isMakingCurrent
              ? '現行に戻しています…'
              : 'このバージョンを現行に戻す'}
          </Button>
        )}
        {hasError && (
          <p className="text-2xs text-destructive">
            レビュー状態を更新できませんでした
          </p>
        )}
      </div>
    </section>
  )
}
