import { Link } from '@tanstack/react-router'

import type {
  ReviewQueueData,
  ReviewQueueItem,
} from '#components/review/review-queue-model'
import { Skeleton } from '#components/ui/skeleton'
import { formatRelative } from '#lib/note-utils'

function ReviewItemLink({ item }: { item: ReviewQueueItem }) {
  const className =
    'flex min-w-0 flex-col gap-1 border-t border-border px-3 py-2.5 first:border-t-0 hover:bg-surface-strong'
  const metadata = (
    <span className="font-mono text-2xs text-muted-foreground">
      {item.kind === 'note'
        ? `ノート · v${String(item.versionNo)}`
        : `アノテーション · ${item.targetSymbol}`}{' '}
      · {formatRelative(item.createdAt)}
    </span>
  )

  if (item.kind === 'note') {
    return (
      <Link
        to="/notes/$noteId"
        params={{ noteId: item.noteId }}
        search={{ version: item.versionNo }}
        className={className}
      >
        <span className="truncate text-sm text-foreground">{item.title}</span>
        {metadata}
      </Link>
    )
  }

  return (
    <Link
      to="/annotations/$annoId"
      params={{ annoId: item.id }}
      className={className}
    >
      <span className="truncate text-sm text-foreground">{item.title}</span>
      {metadata}
    </Link>
  )
}

export function DashboardReviewQueue({
  data,
  isPending,
  isError,
  variant,
}: {
  data: ReviewQueueData | null
  isPending: boolean
  isError: boolean
  variant: 'compact' | 'panel'
}) {
  if (variant === 'compact') {
    return (
      <Link
        to="/review"
        className="flex items-center justify-between gap-3 border border-border bg-card px-3.5 py-2.5 font-mono text-xs hover:border-primary"
      >
        <span className="text-foreground">レビュー待ち</span>
        <span className="flex items-center gap-2 text-muted-foreground">
          <span className="tabular-nums text-foreground">
            {isPending ? '…' : isError ? '—' : String(data?.counts.total ?? 0)}
          </span>
          <span aria-hidden="true" className="text-primary">
            →
          </span>
        </span>
      </Link>
    )
  }

  return (
    <section className="space-y-2" aria-labelledby="dashboard-review-title">
      <div className="flex flex-wrap items-baseline justify-between gap-x-3 gap-y-1">
        <h2
          id="dashboard-review-title"
          className="font-mono text-2xs uppercase tracking-wider text-muted-foreground"
        >
          <span className="mr-2 text-primary">&gt;</span>
          レビュー待ち
        </h2>
        <Link
          to="/review"
          className="font-mono text-2xs text-primary hover:underline"
        >
          レビューへ →
        </Link>
      </div>
      <div className="border border-border bg-card">
        {isPending ? (
          <div className="space-y-2 p-3">
            <Skeleton className="h-4 w-3/4" />
            <Skeleton className="h-10 w-full" />
            <Skeleton className="h-10 w-full" />
          </div>
        ) : isError || data == null ? (
          <p className="px-3.5 py-4 font-mono text-xs text-muted-foreground">
            レビュー待ちを読み込めませんでした。
          </p>
        ) : (
          <>
            <div className="flex flex-wrap gap-x-3 gap-y-1 border-b border-border px-3.5 py-2 font-mono text-2xs text-muted-foreground">
              <span>ノート {data.counts.notes}</span>
              <span>アノテーション {data.counts.annotations}</span>
            </div>
            {data.items.length === 0 ? (
              <p className="px-3.5 py-4 font-mono text-xs text-muted-foreground">
                レビュー待ちはありません。
              </p>
            ) : (
              <div>
                {data.items.slice(0, 3).map((item) => (
                  <ReviewItemLink key={`${item.kind}:${item.id}`} item={item} />
                ))}
              </div>
            )}
          </>
        )}
      </div>
    </section>
  )
}
