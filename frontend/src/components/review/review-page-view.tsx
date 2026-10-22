import { Button } from '@fohte/ui/button'
import { Checkbox } from '@fohte/ui/checkbox'
import { Chip } from '@fohte/ui/chip'
import { SegmentedControl } from '@fohte/ui/segmented-control'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@fohte/ui/select'
import { Link } from '@tanstack/react-router'

import {
  type ReviewNavigationSnapshot,
  type ReviewQueueData,
  type ReviewQueueItem,
  reviewQueueItemKey,
  reviewQueueModel,
  type ReviewTypeFilter,
} from '#components/review/review-queue-model'
import { Skeleton } from '#components/ui/skeleton'
import type { components } from '#lib/api/schema.gen'
import { resolveNoteKindLabel } from '#lib/note-kind-label'
import { formatRelative } from '#lib/note-utils'
import { cn } from '#lib/utils'

type NoteKind = components['schemas']['NoteKind']

export interface ReviewPageViewProps {
  data: ReviewQueueData | null
  noteKinds: NoteKind[]
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
  noteKindFilterOpen?: boolean
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

function getReviewItemDetails(
  item: ReviewQueueItem,
  noteKinds: NoteKind[],
): string[] {
  if (item.kind === 'note') {
    return [
      resolveNoteKindLabel(item.noteKind, noteKinds) ?? '種別なし',
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
  noteKinds: knownNoteKinds,
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
  noteKindFilterOpen = false,
}: ReviewPageViewProps) {
  const {
    visibleItems,
    noteKinds: noteKindKeys,
    visibleKeys,
    allVisibleSelected,
    selectedCount,
  } = reviewQueueModel.buildPage({
    items: data?.items ?? [],
    typeFilter,
    noteKindFilter,
    selectedItemIds,
  })
  const selectedKeys = new Set(selectedItemIds)
  const noteKindItems = [
    { value: 'all', label: 'すべて' },
    ...noteKindKeys.map((kind) => ({
      value: reviewQueueModel.noteKindFilterKey(kind),
      label: resolveNoteKindLabel(kind, knownNoteKinds) ?? '種別なし',
    })),
  ]

  function renderApprovalButton(className: string, mobile = false) {
    return (
      <Button
        type="button"
        size={mobile ? 'lg' : 'sm'}
        disabled={selectedCount === 0 || isApproving}
        onClick={onApproveSelected}
        className={className}
      >
        {isApproving ? '承認中…' : `選択した ${String(selectedCount)} 件を承認`}
      </Button>
    )
  }

  return (
    <section
      className={cn(
        'space-y-4 font-sans text-foreground',
        selectedCount > 0 && 'pb-16 md:pb-0',
      )}
    >
      <header>
        <h1 className="text-2xl font-bold tracking-tight">レビュー</h1>
        <p className="mt-1 text-sm text-muted-foreground">
          未レビューのノートとアノテーションを確認できます。
        </p>
      </header>

      <div className="flex flex-wrap items-end gap-3 border-b border-border">
        <SegmentedControl
          aria-label="レビュー項目の種類"
          value={typeFilter}
          options={[
            {
              value: 'all',
              label: `すべて ${String(data?.counts.total ?? 0)}`,
            },
            {
              value: 'note',
              label: `ノート ${String(data?.counts.notes ?? 0)}`,
            },
            {
              value: 'annotation',
              label: `アノテーション ${String(data?.counts.annotations ?? 0)}`,
            },
          ]}
          onValueChange={onTypeFilterChange}
        />
        <label className="mb-1 ml-auto flex items-center gap-2 font-mono text-xs text-muted-foreground">
          <span>ノート種別</span>
          <Select
            items={noteKindItems}
            value={noteKindFilter}
            disabled={typeFilter === 'annotation'}
            defaultOpen={noteKindFilterOpen}
            onValueChange={(value) => {
              if (value != null) onNoteKindFilterChange(value)
            }}
          >
            <SelectTrigger aria-label="ノート種別で絞り込む">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              {noteKindItems.map(({ value, label }) => (
                <SelectItem key={value} value={value}>
                  {label}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </label>
      </div>

      <div className="flex items-center gap-3 text-xs">
        <label className="inline-flex items-center gap-2 text-muted-foreground">
          <Checkbox
            aria-label="表示中の項目をすべて選択"
            checked={allVisibleSelected}
            disabled={visibleKeys.length === 0 || isApproving}
            onCheckedChange={(checked) => {
              onToggleVisibleItems(visibleKeys, checked)
            }}
          />
          すべて選択
        </label>
        {renderApprovalButton('ml-auto hidden md:inline-flex')}
      </div>

      {hasApprovalError && (
        <p
          role="alert"
          className="border border-destructive px-3 py-2 text-sm text-destructive"
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
          <p className="px-4 py-5 text-sm text-destructive">
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
              const details = getReviewItemDetails(item, knownNoteKinds)
              const reviewNavigation =
                navigationSnapshot == null
                  ? undefined
                  : {
                      reviewQueue: {
                        snapshot: navigationSnapshot,
                        currentItemKey: key,
                      },
                    }
              const linkContent = (
                <>
                  <span className="flex flex-wrap items-center gap-x-2 gap-y-1">
                    <Chip size="md" tone="muted">
                      {getReviewItemTypeLabel(item)}
                    </Chip>
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
                  <Checkbox
                    aria-label={`レビュー項目を選択: ${item.title}`}
                    checked={selectedKeys.has(key)}
                    disabled={isApproving}
                    className="mt-2"
                    onCheckedChange={(checked) => {
                      onToggleItem(key, checked)
                    }}
                  />
                  {itemLink}
                </div>
              )
            })}
          </div>
        )}
      </div>

      {selectedCount > 0 && (
        <div className="fixed inset-x-0 bottom-0 z-10 border-t border-border bg-background px-4 py-3 md:hidden">
          {renderApprovalButton('w-full text-center', true)}
        </div>
      )}
    </section>
  )
}
