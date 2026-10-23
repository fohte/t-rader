import { Button } from '@fohte/ui/button'
import { Checkbox } from '@fohte/ui/checkbox'
import { Chip } from '@fohte/ui/chip'
import { SegmentedControl } from '@fohte/ui/segmented-control'
import { Link } from '@tanstack/react-router'

import {
  type ReviewQueueItem,
  reviewQueueItemKey,
  reviewQueueModel,
} from '#components/review/review-queue-model'
import { StatusPill } from '#components/strategy-home/status-pill'
import { Skeleton } from '#components/ui/skeleton'
import type { components } from '#lib/api/schema.gen'
import { buildSnippet, formatRelative } from '#lib/note-utils'

type Annotation = components['schemas']['Annotation']
export type AnnotationView = 'all' | 'pending'
export type AnnotationListState = 'loading' | 'error' | 'loaded'

export interface AnnotationsListProps {
  annotations: Annotation[]
  view: AnnotationView
  state: AnnotationListState
  kind?: string
  tag?: string
  selectedItemIds: string[]
  isApproving: boolean
  hasApprovalError: boolean
  onViewChange: (view: AnnotationView) => void
  onToggleItem: (itemKey: string, checked: boolean) => void
  onToggleVisibleItems: (itemKeys: string[], checked: boolean) => void
  onApproveSelected: () => void
}

function createNavigationState(
  items: Extract<ReviewQueueItem, { kind: 'annotation' }>[],
  currentItem: Extract<ReviewQueueItem, { kind: 'annotation' }>,
) {
  return {
    snapshot: reviewQueueModel.createNavigationSnapshot(items),
    currentItemKey: reviewQueueItemKey(currentItem),
    items,
    scope: 'annotations' as const,
  }
}

export function AnnotationsList({
  annotations,
  view,
  state,
  kind,
  tag,
  selectedItemIds,
  isApproving,
  hasApprovalError,
  onViewChange,
  onToggleItem,
  onToggleVisibleItems,
  onApproveSelected,
}: AnnotationsListProps) {
  const navigationItems = reviewQueueModel.createAnnotationItems(annotations)
  const itemsById = new Map(
    navigationItems.map((item) => [item.id, item] as const),
  )
  const navigationKeys = navigationItems.map(reviewQueueItemKey)
  const selectedKeys = new Set(selectedItemIds)
  const selectedCount = selectedItemIds.length
  const allVisibleSelected =
    navigationKeys.length > 0 &&
    navigationKeys.every((key) => selectedKeys.has(key))

  return (
    <div className="space-y-3">
      <SegmentedControl
        aria-label="表示するアノテーション"
        value={view}
        options={[
          { value: 'all', label: 'すべて' },
          { value: 'pending', label: '未読・レビュー待ち' },
        ]}
        onValueChange={onViewChange}
      />
      {view === 'pending' && (
        <div className="flex flex-wrap items-center gap-3 border border-border bg-surface-strong px-3.5 py-2">
          <label className="inline-flex items-center gap-2 font-mono text-xs text-muted-foreground-strong">
            <Checkbox
              aria-label="表示中のアノテーションをすべて選択"
              checked={allVisibleSelected}
              disabled={
                state !== 'loaded' || navigationKeys.length === 0 || isApproving
              }
              onCheckedChange={(checked) => {
                onToggleVisibleItems(navigationKeys, checked)
              }}
            />
            表示中を選択
          </label>
          {selectedCount > 0 && (
            <>
              <span className="font-mono text-xs text-muted-foreground-strong">
                {String(selectedCount)} 件を選択中
              </span>
              <Button
                type="button"
                size="sm"
                disabled={state !== 'loaded' || isApproving}
                onClick={onApproveSelected}
                className="ml-auto hidden sm:inline-flex"
              >
                {isApproving
                  ? '承認中…'
                  : `選択した ${String(selectedCount)} 件を承認`}
              </Button>
            </>
          )}
        </div>
      )}
      <section
        className={`border border-border bg-card ${selectedCount > 0 && view === 'pending' ? 'pb-16 sm:pb-0' : ''}`}
      >
        <div className="flex flex-wrap items-center justify-between gap-2 border-b border-border px-3.5 py-2">
          <h2 className="font-mono text-xs font-bold uppercase tracking-wider text-foreground">
            アノテーション一覧
          </h2>
          <span className="font-mono text-2xs text-muted-foreground">
            {String(annotations.length)}
          </span>
        </div>
        {hasApprovalError && (
          <p
            role="alert"
            className="border-b border-border px-3.5 py-2 text-sm text-status-rejected"
          >
            一部の承認に失敗しました。失敗した項目と後続の選択を残しています。
          </p>
        )}
        {state === 'loading' ? (
          <div className="space-y-2 p-3.5" aria-busy="true" role="status">
            <Skeleton className="h-14 w-full" />
            <Skeleton className="h-14 w-full" />
            <Skeleton className="h-14 w-full" />
          </div>
        ) : state === 'error' ? (
          <p
            role="alert"
            className="px-3.5 py-3 font-mono text-xs text-status-rejected"
          >
            アノテーション一覧を読み込めませんでした。
          </p>
        ) : annotations.length === 0 ? (
          <p className="px-3.5 py-3 font-mono text-xs text-muted-foreground">
            {view === 'pending'
              ? '未読・レビュー待ちのアノテーションはありません。'
              : 'アノテーションはありません。'}
          </p>
        ) : (
          <ul>
            {annotations.map((annotation) => {
              const item = itemsById.get(annotation.id)
              const itemKey = reviewQueueItemKey({
                kind: 'annotation',
                id: annotation.id,
              })
              const selected = selectedKeys.has(itemKey)
              return (
                <li
                  key={annotation.id}
                  className="border-b border-border px-3.5 py-3 last:border-b-0"
                >
                  <div className="flex min-w-0 gap-3">
                    {view === 'pending' && (
                      <Checkbox
                        aria-label={`${buildSnippet(annotation.text)} を選択`}
                        checked={selected}
                        disabled={isApproving}
                        onCheckedChange={(checked) => {
                          onToggleItem(itemKey, checked)
                        }}
                        className="mt-1 shrink-0"
                      />
                    )}
                    <div className="min-w-0 flex-1 space-y-1.5">
                      <Link
                        to="/annotations/$annoId"
                        params={{ annoId: annotation.id }}
                        search={{ tab: 'annotations', view, kind, tag }}
                        state={(previous) => ({
                          ...previous,
                          reviewQueue:
                            item == null
                              ? undefined
                              : createNavigationState(navigationItems, item),
                        })}
                        className="block break-words text-sm text-foreground hover:underline"
                      >
                        {buildSnippet(annotation.text)}
                      </Link>
                      <div className="flex min-w-0 flex-wrap items-center gap-2 font-mono text-2xs">
                        <span className="max-w-full break-all">
                          <Chip>{annotation.target_symbol}</Chip>
                        </span>
                        <StatusPill status={annotation.status} />
                        <span className="ml-auto whitespace-nowrap text-muted-foreground">
                          {formatRelative(annotation.updated_at)}
                        </span>
                      </div>
                    </div>
                  </div>
                </li>
              )
            })}
          </ul>
        )}
      </section>
      {selectedCount > 0 && view === 'pending' && (
        <div className="fixed inset-x-0 bottom-0 z-20 border-t border-border bg-card p-3 sm:hidden">
          <Button
            type="button"
            size="lg"
            disabled={state !== 'loaded' || isApproving}
            onClick={onApproveSelected}
            className="w-full"
          >
            {isApproving
              ? '承認中…'
              : `選択した ${String(selectedCount)} 件を承認`}
          </Button>
        </div>
      )}
    </div>
  )
}
