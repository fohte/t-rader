import {
  type ReviewQueueItem,
  reviewQueueModel,
} from '#components/review/review-queue-model'

export type NoteVersionReviewNavigationState =
  | { kind: 'loading' }
  | { kind: 'error' }
  | ReturnType<typeof reviewQueueModel.getNavigation>

interface NoteVersionReviewNavigationViewProps {
  state: NoteVersionReviewNavigationState
  onNavigateToItem: (item: ReviewQueueItem) => void
  onReturnToReview: () => void
}

export function NoteVersionReviewNavigationView({
  state,
  onNavigateToItem,
  onReturnToReview,
}: NoteVersionReviewNavigationViewProps) {
  const previousItem = state.kind === 'item' ? state.previousItem : null

  return (
    <nav
      aria-label="未読レビューの移動"
      className="flex flex-wrap items-center justify-between gap-2 border border-border bg-surface px-4 py-2 font-mono text-xs"
    >
      {state.kind === 'loading' ? (
        <p aria-live="polite" className="text-muted-foreground">
          未読を読み込み中
        </p>
      ) : state.kind === 'error' ? (
        <>
          <p role="alert" className="text-primary">
            未読を読み込めませんでした
          </p>
          <button
            type="button"
            onClick={onReturnToReview}
            className="border border-border px-2.5 py-1 text-muted-foreground-strong hover:border-primary hover:text-primary"
          >
            レビューへ
          </button>
        </>
      ) : state.kind === 'empty' ? (
        <>
          <p className="text-muted-foreground">未読はありません</p>
          <span aria-hidden="true" className="text-muted-foreground">
            ·
          </span>
          <button
            type="button"
            onClick={onReturnToReview}
            className="border border-border px-2.5 py-1 text-muted-foreground-strong hover:border-primary hover:text-primary"
          >
            レビューへ
          </button>
        </>
      ) : state.kind === 'start' ? (
        <>
          <p className="text-muted-foreground">未読 {String(state.total)} 件</p>
          <button
            type="button"
            onClick={() => {
              onNavigateToItem(state.firstItem)
            }}
            className="border border-border px-2.5 py-1 text-muted-foreground-strong hover:border-primary hover:text-primary"
          >
            未読の先頭へ →
          </button>
        </>
      ) : (
        <>
          <p className="text-muted-foreground">未読 {String(state.total)} 件</p>
          <div className="flex flex-wrap items-center gap-2">
            {previousItem != null && (
              <button
                type="button"
                onClick={() => {
                  onNavigateToItem(previousItem)
                }}
                className="border border-border px-2.5 py-1 text-muted-foreground-strong hover:border-primary hover:text-primary"
              >
                ← 前
              </button>
            )}
            <button
              type="button"
              onClick={() => {
                onNavigateToItem(state.nextItem)
              }}
              className="border border-primary px-2.5 py-1 text-primary hover:bg-primary/10"
            >
              次の未読へ →
            </button>
          </div>
        </>
      )}
    </nav>
  )
}
