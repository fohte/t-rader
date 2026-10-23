import { Link } from '@tanstack/react-router'
import type { ReactNode } from 'react'

import {
  type ReviewNavigationTarget,
  reviewQueueItemKey,
  reviewQueueModel,
} from '#components/review/review-queue-model'

type ReviewNavigationBandProps =
  | { isPending: true; scope?: 'notes' }
  | { isError: true; scope?: 'notes' }
  | (ReviewNavigationTarget & { isPending?: false; isError?: false })

function ReviewQueueItemLink({
  item,
  snapshot,
  scope,
  items,
  children,
}: {
  item: ReviewNavigationTarget['items'][number]
  snapshot: ReviewNavigationTarget['snapshot']
  scope?: 'notes'
  items: ReviewNavigationTarget['items']
  children: ReactNode
}) {
  const key = reviewQueueItemKey(item)
  const reviewQueue = {
    snapshot,
    currentItemKey: key,
    ...(scope === 'notes' ? { scope, items } : {}),
  }
  const linkClassName =
    'whitespace-nowrap border border-border px-2.5 py-1 text-muted-foreground-strong hover:border-border-strong hover:text-foreground'

  return item.kind === 'note' ? (
    <Link
      to="/notes/$noteId"
      params={{ noteId: item.noteId }}
      search={(previous) => ({
        ...previous,
        version: item.versionNo,
        version_id: item.id,
      })}
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

export function ReviewNavigationBand(props: ReviewNavigationBandProps) {
  const className =
    'flex flex-wrap items-center gap-x-3 gap-y-1 border-y border-border bg-surface-strong px-4 py-2 font-mono text-xs text-muted-foreground-strong'

  if ('isPending' in props && props.isPending === true) {
    return (
      <div className={className} aria-busy="true" role="status">
        {props.scope === 'notes'
          ? '一覧内の移動情報を読み込んでいます'
          : '未読を読み込んでいます'}
      </div>
    )
  }

  if ('isError' in props && props.isError === true) {
    return (
      <div className={className} role="alert">
        {props.scope === 'notes'
          ? '一覧内の移動情報を読み込めませんでした。'
          : '未読の移動情報を読み込めませんでした。'}
      </div>
    )
  }

  const { items, snapshot, currentItemKey, scope } = props
  const navigation = reviewQueueModel.getNavigation(
    snapshot,
    currentItemKey,
    items,
  )
  if (navigation.kind === 'empty') {
    return (
      <div className={className}>
        {scope === 'notes' ? (
          <span>一覧の対象はありません</span>
        ) : (
          <>
            <span>未読はありません · </span>
            <Link
              to="/review"
              className="whitespace-nowrap text-foreground hover:underline"
            >
              レビューへ
            </Link>
          </>
        )}
      </div>
    )
  }

  if (navigation.kind === 'start') {
    return (
      <div className={className}>
        <span>
          {scope === 'notes' ? '一覧' : '未読'} {String(navigation.total)} 件
          {navigation.firstItem != null && ' · '}
        </span>
        {navigation.firstItem != null && (
          <ReviewQueueItemLink
            item={navigation.firstItem}
            snapshot={snapshot}
            scope={scope}
            items={items}
          >
            {scope === 'notes' ? '先頭へ →' : '未読の先頭へ →'}
          </ReviewQueueItemLink>
        )}
      </div>
    )
  }

  return (
    <div className={className}>
      <span>
        {scope === 'notes' ? '一覧' : 'レビュー'} · 残り{' '}
        {String(navigation.total)} 件
        {(navigation.previousItem != null || navigation.nextItem != null) &&
          ' '}
      </span>
      {(navigation.previousItem != null || navigation.nextItem != null) && (
        <nav
          aria-label={scope === 'notes' ? '一覧内の移動' : '未読の移動'}
          className="ml-auto flex gap-2"
        >
          {navigation.previousItem != null && (
            <ReviewQueueItemLink
              item={navigation.previousItem}
              snapshot={snapshot}
              scope={scope}
              items={items}
            >
              {scope === 'notes' ? '← 前へ' : '← 前の未読'}
            </ReviewQueueItemLink>
          )}
          {navigation.nextItem != null && (
            <ReviewQueueItemLink
              item={navigation.nextItem}
              snapshot={snapshot}
              scope={scope}
              items={items}
            >
              {scope === 'notes' ? '次へ →' : '次の未読へ →'}
            </ReviewQueueItemLink>
          )}
        </nav>
      )}
    </div>
  )
}
