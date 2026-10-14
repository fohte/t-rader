import { Link } from '@tanstack/react-router'
import type { ReactNode } from 'react'

import {
  type ReviewNavigationTarget,
  reviewQueueItemKey,
  reviewQueueModel,
} from '#components/review/review-queue-model'
import type { ReviewNavigationViewState } from '#components/review/use-review-navigation'

type ReviewNavigationBandProps = {
  state: Pick<ReviewNavigationViewState, 'navigation' | 'isPending' | 'isError'>
}

function ReviewQueueItemLink({
  item,
  snapshot,
  children,
}: {
  item: ReviewNavigationTarget['items'][number]
  snapshot: ReviewNavigationTarget['snapshot']
  children: ReactNode
}) {
  const key = reviewQueueItemKey(item)
  const reviewQueue = reviewQueueModel.createNavigationState(snapshot, key)
  const linkClassName =
    'whitespace-nowrap border border-border px-2.5 py-1 text-muted-foreground-strong hover:border-primary hover:text-primary'

  return item.kind === 'note' ? (
    <Link
      to="/notes/$noteId"
      params={{ noteId: item.noteId }}
      search={{ version: item.versionNo }}
      state={(previous) => ({ ...previous, reviewQueue })}
      className={linkClassName}
    >
      {children}
    </Link>
  ) : (
    <Link
      to="/annotations/$annoId"
      params={{ annoId: item.id }}
      state={(previous) => ({ ...previous, reviewQueue })}
      className={linkClassName}
    >
      {children}
    </Link>
  )
}

export function ReviewNavigationBand({ state }: ReviewNavigationBandProps) {
  const className =
    'flex flex-wrap items-center gap-x-3 gap-y-1 border-y border-border bg-surface-strong px-4 py-2 font-mono text-xs text-muted-foreground-strong'

  const stateNavigation = state.navigation
  if (stateNavigation == null) {
    if (state.isError) {
      return (
        <div className={className} role="alert">
          未読の移動情報を読み込めませんでした。
        </div>
      )
    }
    if (state.isPending) {
      return (
        <div className={className} aria-busy="true" role="status">
          未読を読み込んでいます
        </div>
      )
    }
    return null
  }

  const { items, snapshot, currentItemKey } = stateNavigation
  const queueNavigation = reviewQueueModel.getNavigation(
    snapshot,
    currentItemKey,
    items,
  )
  if (queueNavigation.kind === 'empty') {
    return (
      <div className={className}>
        <span>未読はありません · </span>
        <Link
          to="/review"
          className="whitespace-nowrap text-primary hover:underline"
        >
          レビューへ
        </Link>
      </div>
    )
  }

  if (queueNavigation.kind === 'start') {
    return (
      <div className={className}>
        <span>
          未読 {String(queueNavigation.total)} 件
          {queueNavigation.firstItem != null && ' · '}
        </span>
        {queueNavigation.firstItem != null && (
          <ReviewQueueItemLink
            item={queueNavigation.firstItem}
            snapshot={snapshot}
          >
            未読の先頭へ →
          </ReviewQueueItemLink>
        )}
      </div>
    )
  }

  return (
    <div className={className}>
      <span>
        レビュー · 残り {String(queueNavigation.total)} 件
        {(queueNavigation.previousItem != null ||
          queueNavigation.nextItem != null) &&
          ' '}
      </span>
      {(queueNavigation.previousItem != null ||
        queueNavigation.nextItem != null) && (
        <nav aria-label="未読の移動" className="ml-auto flex gap-2">
          {queueNavigation.previousItem != null && (
            <ReviewQueueItemLink
              item={queueNavigation.previousItem}
              snapshot={snapshot}
            >
              ← 前の未読
            </ReviewQueueItemLink>
          )}
          {queueNavigation.nextItem != null && (
            <ReviewQueueItemLink
              item={queueNavigation.nextItem}
              snapshot={snapshot}
            >
              次の未読へ →
            </ReviewQueueItemLink>
          )}
        </nav>
      )}
    </div>
  )
}
