import { Link } from '@tanstack/react-router'
import type { ReactNode } from 'react'

import {
  type ReviewNavigationSnapshot,
  type ReviewQueueItem,
  reviewQueueItemKey,
  reviewQueueModel,
} from '#components/review/review-queue-model'

interface ReviewNavigationBandProps {
  items: ReviewQueueItem[]
  snapshot: ReviewNavigationSnapshot
  currentItemKey: string
}

function ReviewQueueItemLink({
  item,
  snapshot,
  children,
}: {
  item: ReviewQueueItem
  snapshot: ReviewNavigationSnapshot
  children: ReactNode
}) {
  const key = reviewQueueItemKey(item)
  const reviewQueue = { snapshot, currentItemKey: key }
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

export function ReviewNavigationBand({
  items,
  snapshot,
  currentItemKey,
}: ReviewNavigationBandProps) {
  const navigation = reviewQueueModel.getNavigation(
    snapshot,
    currentItemKey,
    items,
  )
  const className =
    'flex flex-wrap items-center gap-x-3 gap-y-1 border-y border-border bg-surface-strong px-4 py-2 font-mono text-xs text-muted-foreground-strong'

  if (navigation.kind === 'empty') {
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

  if (navigation.kind === 'start') {
    return (
      <div className={className}>
        <span>未読 {String(navigation.total)} 件 · </span>
        <ReviewQueueItemLink item={navigation.firstItem} snapshot={snapshot}>
          未読の先頭へ →
        </ReviewQueueItemLink>
      </div>
    )
  }

  return (
    <div className={className}>
      <span>レビュー · 残り {String(navigation.total)} 件 </span>
      <nav aria-label="未読の移動" className="ml-auto flex gap-2">
        {navigation.previousItem != null && (
          <ReviewQueueItemLink
            item={navigation.previousItem}
            snapshot={snapshot}
          >
            ← 前の未読
          </ReviewQueueItemLink>
        )}
        <ReviewQueueItemLink item={navigation.nextItem} snapshot={snapshot}>
          次の未読へ →
        </ReviewQueueItemLink>
      </nav>
    </div>
  )
}
