import { Button } from '@fohte/ui/button'
import { Checkbox } from '@fohte/ui/checkbox'
import { Chip } from '@fohte/ui/chip'
import { Link } from '@tanstack/react-router'

import { NoteKindLabel } from '#components/notes/note-kind-label'
import {
  type ReviewQueueItem,
  reviewQueueItemKey,
  reviewQueueModel,
} from '#components/review/review-queue-model'
import {
  type NoteOverview,
  type NoteOverviewView,
  notesOverviewModel,
} from '#components/strategy-home/notes-overview-model'
import { StatusPill } from '#components/strategy-home/status-pill'
import { Skeleton } from '#components/ui/skeleton'
import type { components } from '#lib/api/schema.gen'
import type { NoteKindCatalogState } from '#lib/note-kind-label'
import { formatRelative } from '#lib/note-utils'

type NoteKind = components['schemas']['NoteKind']

export interface NotesListProps {
  overviews: NoteOverview[]
  view: NoteOverviewView
  kind?: string
  tag?: string
  state: 'loading' | 'error' | 'loaded'
  noteKinds: readonly NoteKind[]
  noteKindCatalogState: NoteKindCatalogState
  selectedItemIds: string[]
  selectedItems: ReviewQueueItem[]
  isApproving: boolean
  hasApprovalError: boolean
  onToggleItem: (itemKey: string, checked: boolean) => void
  onApproveSelected: () => void
}

function createNavigationState(
  items: Extract<ReviewQueueItem, { kind: 'note' }>[],
  currentItem: Extract<ReviewQueueItem, { kind: 'note' }>,
) {
  const scopedItems = items.map((item) =>
    item.noteId === currentItem.noteId ? currentItem : item,
  )
  return {
    snapshot: reviewQueueModel.createNavigationSnapshot(scopedItems),
    currentItemKey: reviewQueueItemKey(currentItem),
    items: scopedItems,
    scope: 'notes' as const,
  }
}

export function NotesList({
  overviews,
  view,
  kind,
  tag,
  state,
  noteKinds,
  noteKindCatalogState,
  selectedItemIds,
  selectedItems,
  isApproving,
  hasApprovalError,
  onToggleItem,
  onApproveSelected,
}: NotesListProps) {
  const navigationItems = notesOverviewModel.createNavigationItems(
    overviews,
    view,
  )
  const selectedCount = selectedItemIds.length
  const selectedByNoteId = new Map(
    selectedItems.flatMap((item) =>
      item.kind === 'note' ? [[item.noteId, item] as const] : [],
    ),
  )

  return (
    <section className="border border-border bg-card">
      <div className="flex flex-wrap items-center justify-between gap-2 border-b border-border px-3.5 py-2">
        <h3 className="font-mono text-xs font-bold uppercase tracking-wider text-foreground">
          ノート一覧
        </h3>
        <span className="font-mono text-2xs text-muted-foreground">
          {overviews.length}
        </span>
      </div>
      {selectedCount > 0 && (
        <div className="flex items-center gap-3 border-b border-border bg-surface-strong px-3.5 py-2">
          <span className="font-mono text-xs text-muted-foreground-strong">
            {String(selectedCount)} 件を選択中
          </span>
          <Button
            type="button"
            size="sm"
            disabled={isApproving}
            onClick={onApproveSelected}
            className="ml-auto hidden sm:inline-flex"
          >
            {isApproving
              ? '承認中…'
              : `選択した ${String(selectedCount)} 件を承認`}
          </Button>
        </div>
      )}
      {hasApprovalError && (
        <p
          role="alert"
          className="border-b border-border px-3.5 py-2 text-sm text-status-rejected"
        >
          承認に失敗しました。失敗したノートと後続の選択は残っています。
        </p>
      )}
      {state === 'loading' ? (
        <div className="space-y-2 p-3.5" aria-busy="true" role="status">
          <Skeleton className="h-12 w-full" />
          <Skeleton className="h-12 w-full" />
          <Skeleton className="h-12 w-full" />
        </div>
      ) : state === 'error' ? (
        <p
          role="alert"
          className="px-3.5 py-3 font-mono text-xs text-status-rejected"
        >
          ノート一覧を読み込めませんでした。
        </p>
      ) : overviews.length === 0 ? (
        <p className="px-3.5 py-3 font-mono text-xs text-muted-foreground">
          {view === 'pending'
            ? '未読・レビュー待ちのノートはありません。'
            : 'ノートはありません。'}
        </p>
      ) : (
        <ul>
          {overviews.map((overview) => {
            const note = overview.representative
            const selectionKey = notesOverviewModel.selectionKey(note.id)
            const selectedItem = selectedByNoteId.get(note.id)
            const latestUnread = overview.latest_unread
            const representativeTarget = navigationItems.find(
              (item) => item.noteId === note.id,
            )
            const unreadTarget = notesOverviewModel.createReviewItem(overview)
            const defaultTarget =
              view === 'pending' ? unreadTarget : representativeTarget
            const titleTarget = defaultTarget ?? representativeTarget
            const titleVersionId = titleTarget?.id ?? note.version_id
            const titleVersionNo = titleTarget?.versionNo ?? note.version_no
            const selectedLatestVersionDiffers =
              selectedItem?.kind === 'note' &&
              latestUnread != null &&
              selectedItem.id !== latestUnread.id

            return (
              <li
                key={note.id}
                className="border-b border-border px-3.5 py-3 last:border-b-0"
              >
                <div className="flex min-w-0 gap-3">
                  <Checkbox
                    aria-label={
                      selectedItem == null
                        ? `${note.title} の最新未読バージョンを選択`
                        : `${note.title} の v${String(selectedItem.versionNo)} 選択を解除`
                    }
                    checked={selectedItem != null}
                    disabled={
                      isApproving ||
                      (latestUnread == null && selectedItem == null)
                    }
                    onCheckedChange={(checked) => {
                      onToggleItem(selectionKey, checked)
                    }}
                    className="mt-1 shrink-0"
                  />
                  <div className="min-w-0 flex-1 space-y-1.5">
                    <Link
                      to="/notes/$noteId"
                      params={{ noteId: note.id }}
                      search={{
                        version: titleVersionNo,
                        version_id: titleVersionId,
                        view,
                        kind,
                        tag,
                      }}
                      state={(previous) => ({
                        ...previous,
                        reviewQueue:
                          titleTarget == null
                            ? undefined
                            : createNavigationState(
                                navigationItems,
                                titleTarget,
                              ),
                      })}
                      className="block break-words text-sm text-foreground hover:underline"
                    >
                      {note.title}
                    </Link>
                    <div className="flex min-w-0 flex-wrap items-center gap-2 font-mono text-2xs">
                      <NoteKindLabel
                        kind={note.kind ?? null}
                        noteKinds={noteKinds}
                        catalogState={noteKindCatalogState}
                      />
                      <Chip>
                        {note.is_current
                          ? `現行 v${String(note.version_no)}`
                          : `代表 v${String(note.version_no)} · 非現行`}
                      </Chip>
                      <StatusPill status={note.status} />
                      {overview.unread_version_count > 0 && (
                        <Chip>
                          未読 {String(overview.unread_version_count)} 版
                        </Chip>
                      )}
                      {!note.is_current && overview.current != null && (
                        <>
                          <Chip>
                            現行 v{String(overview.current.version_no)}
                          </Chip>
                          <StatusPill status={overview.current.status} />
                        </>
                      )}
                      {!note.is_current && overview.current == null && (
                        <Chip>現行なし</Chip>
                      )}
                      {selectedLatestVersionDiffers && (
                        <Chip>v{String(selectedItem.versionNo)} を選択中</Chip>
                      )}
                    </div>
                    <div className="flex flex-wrap items-center gap-2 font-mono text-2xs">
                      {note.tags.map((noteTag) => (
                        <Chip key={noteTag}>{noteTag}</Chip>
                      ))}
                      <span className="ml-auto text-muted-foreground">
                        {formatRelative(note.updated_at)}
                      </span>
                    </div>
                    {latestUnread != null &&
                      latestUnread.id !== titleVersionId &&
                      unreadTarget != null && (
                        <Link
                          to="/notes/$noteId"
                          params={{ noteId: note.id }}
                          search={{
                            version: unreadTarget.versionNo,
                            version_id: unreadTarget.id,
                            view,
                            kind,
                            tag,
                          }}
                          state={(previous) => ({
                            ...previous,
                            reviewQueue: createNavigationState(
                              navigationItems,
                              unreadTarget,
                            ),
                          })}
                          className="inline-flex min-h-8 items-center text-xs text-primary hover:underline"
                        >
                          最新未読 v{String(latestUnread.version_no)}{' '}
                          を確認・レビュー
                        </Link>
                      )}
                    {selectedLatestVersionDiffers && (
                      <p className="text-2xs text-muted-foreground">
                        承認対象は選択時の v{String(selectedItem.versionNo)}{' '}
                        です。
                      </p>
                    )}
                  </div>
                </div>
              </li>
            )
          })}
        </ul>
      )}
      {selectedCount > 0 && (
        <div className="fixed inset-x-0 bottom-0 z-20 border-t border-border bg-card p-3 sm:hidden">
          <Button
            type="button"
            size="lg"
            disabled={isApproving}
            onClick={onApproveSelected}
            className="w-full"
          >
            {isApproving
              ? '承認中…'
              : `選択した ${String(selectedCount)} 件を承認`}
          </Button>
        </div>
      )}
    </section>
  )
}
