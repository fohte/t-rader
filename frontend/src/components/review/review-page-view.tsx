import { Link } from '@tanstack/react-router'

import {
  getReviewNavigation,
  type ReviewNavigationSnapshot,
  type ReviewQueueData,
  type ReviewQueueItem,
  reviewQueueItemKey,
} from '#components/review/review-queue-model'
import { Skeleton } from '#components/ui/skeleton'
import { formatRelative } from '#lib/note-utils'

export type ReviewTypeFilter = 'all' | 'note' | 'annotation'

function reviewNoteKindFilterKey(kind: string | null): string {
  return kind == null ? 'none' : `value:${encodeURIComponent(kind)}`
}

export interface ReviewPageViewProps {
  data: ReviewQueueData | null
  navigationSnapshot: ReviewNavigationSnapshot | null
  isPending: boolean
  isError: boolean
  typeFilter: ReviewTypeFilter
  noteKindFilter: string
  selectedItemIds: string[]
  isApproving: boolean
  hasApprovalError: boolean
  onTypeFilterChange: (filter: ReviewTypeFilter) => void
  onNoteKindFilterChange: (filter: string) => void
  onToggleItem: (itemKey: string, checked: boolean) => void
  onToggleVisibleItems: (itemKeys: string[], checked: boolean) => void
  onApproveSelected: () => void
}

function formatAnnotationDate(value: string): string {
  const date = new Date(value)
  if (Number.isNaN(date.getTime())) return value
  return new Intl.DateTimeFormat('ja-JP', {
    year: 'numeric',
    month: '2-digit',
    day: '2-digit',
  }).format(date)
}

function formatAnnotationPrice(value: number | null): string | null {
  if (value == null) return null
  return new Intl.NumberFormat('ja-JP', { maximumFractionDigits: 2 }).format(
    value,
  )
}

function getReviewItemTypeLabel(item: ReviewQueueItem): string {
  if (item.kind === 'annotation') return 'アノテーション'
  return item.versionNo === 1
    ? '新規ノート'
    : `ノート更新 v${String(item.versionNo)}`
}

function getReviewItemDetails(item: ReviewQueueItem): string[] {
  if (item.kind === 'note') {
    return [
      item.noteKind ?? '種別なし',
      item.createdByKind === 'llm' ? 'エージェント' : 'ユーザー',
      ...(item.unreadVersionCount > 1
        ? [`未読 ${String(item.unreadVersionCount)} 版`]
        : []),
    ]
  }

  const price = formatAnnotationPrice(item.price)
  return [
    item.targetSymbol,
    formatAnnotationDate(item.timestamp),
    ...(price == null ? [] : [price]),
  ]
}

export function ReviewPageView({
  data,
  navigationSnapshot,
  isPending,
  isError,
  typeFilter,
  noteKindFilter,
  selectedItemIds,
  isApproving,
  hasApprovalError,
  onTypeFilterChange,
  onNoteKindFilterChange,
  onToggleItem,
  onToggleVisibleItems,
  onApproveSelected,
}: ReviewPageViewProps) {
  const selectedKeys = new Set(selectedItemIds)
  const visibleItems = (data?.items ?? []).filter((item) => {
    const matchesType = typeFilter === 'all' || item.kind === typeFilter
    const matchesNoteKind =
      item.kind !== 'note' ||
      noteKindFilter === 'all' ||
      reviewNoteKindFilterKey(item.noteKind) === noteKindFilter
    return matchesType && matchesNoteKind
  })
  const noteKinds = [
    ...new Set(
      (data?.items ?? [])
        .filter(
          (item): item is Extract<ReviewQueueItem, { kind: 'note' }> =>
            item.kind === 'note',
        )
        .map((item) => item.noteKind),
    ),
  ]
  const visibleKeys = visibleItems.map(reviewQueueItemKey)
  const allVisibleSelected =
    visibleKeys.length > 0 && visibleKeys.every((key) => selectedKeys.has(key))
  const selectedCount = (data?.items ?? []).filter((item) =>
    selectedKeys.has(reviewQueueItemKey(item)),
  ).length
  const bottomPaddingClass = selectedCount > 0 ? 'pb-16 md:pb-0' : ''

  function renderApprovalButton(className: string) {
    return (
      <button
        type="button"
        disabled={selectedCount === 0 || isApproving}
        onClick={onApproveSelected}
        className={className}
      >
        {isApproving ? '承認中…' : `選択した ${String(selectedCount)} 件を承認`}
      </button>
    )
  }

  return (
    <section
      className={`space-y-4 font-sans text-foreground ${bottomPaddingClass}`}
    >
      <header>
        <h1 className="text-2xl font-bold tracking-tight">レビュー</h1>
        <p className="mt-1 text-sm text-muted-foreground">
          未レビューのノートとアノテーションを確認できます。
        </p>
      </header>

      <div className="flex flex-wrap items-end gap-3 border-b border-border">
        <div role="tablist" aria-label="レビュー項目の種類" className="flex">
          {(
            [
              ['all', 'すべて', data?.counts.total ?? 0],
              ['note', 'ノート', data?.counts.notes ?? 0],
              ['annotation', 'アノテーション', data?.counts.annotations ?? 0],
            ] as const
          ).map(([filter, label, count]) => (
            <button
              key={filter}
              type="button"
              role="tab"
              aria-selected={typeFilter === filter}
              onClick={() => {
                onTypeFilterChange(filter)
              }}
              className={`border-b-2 px-3 py-2 text-sm ${
                typeFilter === filter
                  ? 'border-primary text-foreground'
                  : 'border-transparent text-muted-foreground hover:text-foreground'
              }`}
            >
              {label}{' '}
              <span className="font-mono text-2xs">{String(count)}</span>
            </button>
          ))}
        </div>
        <label className="mb-1 ml-auto flex items-center gap-2 text-xs text-muted-foreground">
          ノート種別
          <select
            aria-label="ノート種別で絞り込む"
            value={noteKindFilter}
            disabled={typeFilter === 'annotation'}
            onChange={(event) => {
              onNoteKindFilterChange(event.currentTarget.value)
            }}
            className="border border-border bg-card px-2 py-1 text-foreground disabled:opacity-50"
          >
            <option value="all">すべて</option>
            {noteKinds.map((kind) => {
              const value = reviewNoteKindFilterKey(kind)
              return (
                <option key={value} value={value}>
                  {kind ?? '種別なし'}
                </option>
              )
            })}
          </select>
        </label>
      </div>

      <div className="flex items-center gap-3 text-xs">
        <label className="inline-flex items-center gap-2 text-muted-foreground">
          <input
            type="checkbox"
            aria-label="表示中の項目をすべて選択"
            checked={allVisibleSelected}
            disabled={visibleKeys.length === 0 || isApproving}
            onChange={(event) => {
              onToggleVisibleItems(visibleKeys, event.currentTarget.checked)
            }}
            className="accent-primary"
          />
          すべて選択
        </label>
        {renderApprovalButton(
          'ml-auto hidden border border-primary px-3 py-1.5 text-primary hover:bg-primary/10 disabled:cursor-not-allowed disabled:opacity-50 md:inline-flex',
        )}
      </div>

      {hasApprovalError && (
        <p
          role="alert"
          className="border border-primary px-3 py-2 text-sm text-primary"
        >
          一部の項目を承認できませんでした。未読一覧を更新しました。
        </p>
      )}

      <div className="border border-border bg-card">
        {isPending ? (
          <div className="space-y-2 p-3">
            <Skeleton className="h-12 w-full" />
            <Skeleton className="h-12 w-full" />
            <Skeleton className="h-12 w-full" />
          </div>
        ) : isError ? (
          <p className="px-4 py-5 text-sm text-primary">
            レビュー項目を読み込めませんでした。
          </p>
        ) : data == null || data.items.length === 0 ? (
          <p className="px-4 py-5 text-sm text-muted-foreground">
            未レビューの提案はありません。
          </p>
        ) : visibleItems.length === 0 ? (
          <p className="px-4 py-5 text-sm text-muted-foreground">
            条件に合う未レビューの提案はありません。
          </p>
        ) : (
          <div>
            {visibleItems.map((item) => {
              const key = reviewQueueItemKey(item)
              const details = getReviewItemDetails(item)
              const reviewNavigation =
                navigationSnapshot == null
                  ? undefined
                  : {
                      reviewQueue: {
                        snapshot: navigationSnapshot,
                        currentItemKey: key,
                        navigation: getReviewNavigation(
                          navigationSnapshot,
                          key,
                          data.items,
                        ),
                      },
                    }
              const linkContent = (
                <>
                  <span className="flex flex-wrap items-center gap-x-2 gap-y-1">
                    <span className="border border-border px-1.5 py-0.5 font-mono text-2xs text-muted-foreground-strong">
                      {getReviewItemTypeLabel(item)}
                    </span>
                    <span className="min-w-0 flex-1 text-sm font-semibold text-foreground">
                      {item.title}
                    </span>
                    <span className="ml-auto whitespace-nowrap font-mono text-2xs text-muted-foreground">
                      {formatRelative(item.createdAt)}
                    </span>
                  </span>
                  <span className="mt-1 flex flex-wrap gap-x-2 font-mono text-2xs text-muted-foreground">
                    {details.map((detail, index) => (
                      <span key={`${key}:${String(index)}`}>
                        {index > 0 ? '· ' : ''}
                        {detail}
                      </span>
                    ))}
                  </span>
                </>
              )
              const itemLink =
                item.kind === 'note' ? (
                  <Link
                    to="/notes/$noteId"
                    params={{ noteId: item.noteId }}
                    search={{ version: item.versionNo }}
                    state={
                      reviewNavigation == null
                        ? undefined
                        : (previous) => ({ ...previous, ...reviewNavigation })
                    }
                    className="min-w-0 flex-1 px-2 py-1.5 hover:bg-surface-strong"
                  >
                    {linkContent}
                  </Link>
                ) : (
                  <Link
                    to="/annotations/$annoId"
                    params={{ annoId: item.id }}
                    state={
                      reviewNavigation == null
                        ? undefined
                        : (previous) => ({ ...previous, ...reviewNavigation })
                    }
                    className="min-w-0 flex-1 px-2 py-1.5 hover:bg-surface-strong"
                  >
                    {linkContent}
                  </Link>
                )
              return (
                <div
                  key={key}
                  className="flex items-start gap-2 border-t border-border px-3 py-2.5 first:border-t-0"
                >
                  <input
                    type="checkbox"
                    aria-label={`レビュー項目を選択: ${item.title}`}
                    checked={selectedKeys.has(key)}
                    disabled={isApproving}
                    onChange={(event) => {
                      onToggleItem(key, event.currentTarget.checked)
                    }}
                    className="mt-2 accent-primary"
                  />
                  {itemLink}
                </div>
              )
            })}
          </div>
        )}
      </div>

      {selectedCount > 0 &&
        renderApprovalButton(
          'fixed inset-x-0 bottom-0 z-10 border-t border-border bg-background px-4 py-3 text-center text-sm text-primary disabled:cursor-not-allowed disabled:opacity-50 md:hidden',
        )}
    </section>
  )
}
