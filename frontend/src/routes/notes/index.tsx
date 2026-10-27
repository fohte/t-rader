import { useQueryClient } from '@tanstack/react-query'
import { createFileRoute } from '@tanstack/react-router'
import { useEffect, useState } from 'react'

import { AnnotationsList } from '#components/annotations/annotations-list'
import { ReviewLink } from '#components/note-detail/review-link'
import {
  type NotesTab,
  NotesTabs,
  notesTabSearch,
} from '#components/notes/notes-tabs'
import {
  type ReviewQueueData,
  type ReviewQueueItem,
  reviewQueueItemKey,
  reviewQueueModel,
} from '#components/review/review-queue-model'
import { getReviewQueueQueryKeys } from '#components/review/review-queue-query-keys'
import {
  type ReviewApprovalCallbacks,
  useReviewApproval,
} from '#components/review/use-review-approval'
import { CreateNoteDialog } from '#components/strategy-home/create-note-dialog'
import { NotesFilters } from '#components/strategy-home/notes-filters'
import { NotesList } from '#components/strategy-home/notes-list'
import {
  type NoteOverviewView,
  notesOverviewModel,
} from '#components/strategy-home/notes-overview-model'
import { $api } from '#lib/api/client'
import type { components } from '#lib/api/schema.gen'

type NoteOverview = components['schemas']['NoteOverviewResponse']
type Annotation = components['schemas']['Annotation']

export const Route = createFileRoute('/notes/')({
  validateSearch: (
    search: Record<string, unknown>,
  ): {
    view?: NoteOverviewView
    kind?: string
    tag?: string
    tab?: NotesTab
  } => ({
    ...(search.view === 'pending' || search.view === 'all'
      ? { view: search.view }
      : {}),
    kind:
      typeof search.kind === 'string' && search.kind.length > 0
        ? search.kind
        : undefined,
    tag:
      typeof search.tag === 'string' && search.tag.length > 0
        ? search.tag
        : undefined,
    ...notesTabSearch(search.tab === 'annotations' ? 'annotations' : 'notes'),
  }),
  component: NotesPage,
})

function getSelectionKey(item: ReviewQueueItem): string {
  return item.kind === 'note'
    ? notesOverviewModel.selectionKey(item.noteId)
    : reviewQueueItemKey(item)
}

function createReviewData(
  overviews: NoteOverview[] | undefined,
): ReviewQueueData | null {
  if (overviews == null) return null
  const items = overviews.flatMap((overview) => {
    const item = notesOverviewModel.createReviewItem(overview)
    return item == null ? [] : [item]
  })
  return {
    counts: { notes: items.length, annotations: 0, total: items.length },
    items,
  }
}

function createAnnotationReviewData(
  annotations: Annotation[] | undefined,
): ReviewQueueData | null {
  if (annotations == null) return null
  const items = reviewQueueModel.createAnnotationItems(annotations)
  return {
    counts: { notes: 0, annotations: items.length, total: items.length },
    items,
  }
}

function NotesPage() {
  const { view: requestedView, kind, tag, tab } = Route.useSearch()
  const navigate = Route.useNavigate()
  const queryClient = useQueryClient()
  const view = requestedView ?? 'all'
  const activeTab = tab ?? 'notes'
  const showingNotes = activeTab === 'notes'
  const {
    data: noteKinds,
    isPending: noteKindsPending,
    isError: noteKindsError,
  } = $api.useQuery('get', '/api/note-kinds', undefined, {
    enabled: showingNotes,
  })
  const { data: allTagOverviews, isError: allTagOverviewsError } =
    $api.useQuery(
      'get',
      '/api/notes/overview',
      { params: { query: { view: 'all' } } },
      { enabled: showingNotes },
    )
  const { data: pendingTagOverviews, isError: pendingTagOverviewsError } =
    $api.useQuery(
      'get',
      '/api/notes/overview',
      { params: { query: { view: 'pending' } } },
      { enabled: showingNotes },
    )
  const {
    data: overviews,
    isPending: overviewsPending,
    isError: overviewsError,
  } = $api.useQuery(
    'get',
    '/api/notes/overview',
    { params: { query: { view, kind, tag } } },
    { enabled: showingNotes },
  )
  const {
    data: annotations,
    isPending: annotationsPending,
    isError: annotationsError,
  } = $api.useQuery(
    'get',
    '/api/annotations',
    view === 'pending'
      ? { params: { query: { status: 'unread' } } }
      : undefined,
    { enabled: !showingNotes },
  )
  const {
    data: unreadAnnotations,
    isPending: unreadAnnotationsPending,
    isError: unreadAnnotationsError,
  } = $api.useQuery('get', '/api/annotations', {
    params: { query: { status: 'unread' } },
  })
  const approveVersion = $api.useMutation(
    'post',
    '/api/notes/{id}/versions/{n}/approve',
  )
  const approveAnnotation = $api.useMutation(
    'post',
    '/api/annotations/{id}/approve',
  )
  const [
    confirmedWarningDispositionVersionIds,
    setConfirmedWarningDispositionVersionIds,
  ] = useState<string[]>([])
  const confirmedWarningVersionIds = new Set(
    confirmedWarningDispositionVersionIds,
  )
  const tagSet = new Set(
    [...(allTagOverviews ?? []), ...(pendingTagOverviews ?? [])].flatMap(
      (overview) => overview.representative.tags,
    ),
  )
  if (tag != null) tagSet.add(tag)
  const tags = [...tagSet].sort((left, right) =>
    left.localeCompare(right, 'ja'),
  )
  const reviewData = showingNotes
    ? createReviewData(overviews)
    : view === 'pending'
      ? createAnnotationReviewData(annotations)
      : null

  function invalidateReviewData(): void {
    for (const queryKey of getReviewQueueQueryKeys()) {
      void queryClient.invalidateQueries({ queryKey })
    }
    void queryClient.invalidateQueries({
      queryKey: $api.queryOptions('get', '/api/notes').queryKey,
    })
    void queryClient.invalidateQueries({
      queryKey: $api.queryOptions('get', '/api/annotations').queryKey,
    })
  }

  function approveItem(
    item: ReviewQueueItem,
    callbacks: ReviewApprovalCallbacks,
  ): void {
    if (item.kind === 'annotation') {
      approveAnnotation.mutate(
        { params: { path: { id: item.id } }, body: {} },
        callbacks,
      )
      return
    }
    approveVersion.mutate(
      {
        params: { path: { id: item.noteId, n: item.versionNo } },
        body:
          (item.warningDispositions?.length ?? 0) > 0 &&
          confirmedWarningVersionIds.has(item.id)
            ? { warning_dispositions_confirmed: true }
            : {},
      },
      {
        onSuccess: () => {
          setConfirmedWarningDispositionVersionIds((confirmed) =>
            confirmed.filter((versionId) => versionId !== item.id),
          )
          callbacks.onSuccess()
        },
        onError: callbacks.onError,
      },
    )
  }

  const approval = useReviewApproval({
    data: reviewData,
    approveItem,
    invalidate: invalidateReviewData,
    getSelectionKey,
    preserveSelectedItems: true,
  })
  const clearSelection = approval.actions.clearSelection
  useEffect(() => {
    clearSelection()
    setConfirmedWarningDispositionVersionIds([])
  }, [activeTab, clearSelection, kind, tag, view])

  function onToggleReviewItem(itemKey: string, checked: boolean): void {
    if (!checked) {
      const selectedItem = approval.selectedItems.find(
        (item) => getSelectionKey(item) === itemKey,
      )
      if (selectedItem?.kind === 'note') {
        setConfirmedWarningDispositionVersionIds((confirmed) =>
          confirmed.filter((versionId) => versionId !== selectedItem.id),
        )
      }
    }
    approval.actions.toggleItem(itemKey, checked)
  }

  function confirmWarningDisposition(
    versionId: string,
    confirmed: boolean,
  ): void {
    setConfirmedWarningDispositionVersionIds((current) =>
      confirmed
        ? [...new Set([...current, versionId])]
        : current.filter((id) => id !== versionId),
    )
  }

  function approveSelected(): void {
    const hasWarningDispositions = approval.selectedItems.some(
      (item) =>
        item.kind === 'note' && (item.warningDispositions?.length ?? 0) > 0,
    )
    if (hasWarningDispositions) return
    approval.actions.approveSelected()
  }

  function approveWarningVersion(itemKey: string): void {
    const item = approval.selectedItems.find(
      (candidate) => getSelectionKey(candidate) === itemKey,
    )
    if (
      item?.kind !== 'note' ||
      (item.warningDispositions?.length ?? 0) === 0 ||
      !confirmedWarningVersionIds.has(item.id)
    ) {
      return
    }
    approval.actions.approveOne(itemKey)
  }

  return (
    <div className="space-y-4 font-sans text-foreground">
      <header className="flex flex-wrap items-center justify-between gap-3">
        <h1 className="text-2xl font-bold leading-tight tracking-tight">
          <span className="font-mono font-bold text-primary">&gt;</span>{' '}
          {showingNotes ? 'ノート' : 'アノテーション'}
        </h1>
        {showingNotes && (
          <div className="flex flex-wrap items-center gap-2">
            <CreateNoteDialog />
            <ReviewLink />
          </div>
        )}
      </header>
      <NotesTabs
        activeTab={activeTab}
        annotationUnreadCount={unreadAnnotations?.length}
        annotationUnreadCountState={
          unreadAnnotationsPending
            ? 'loading'
            : unreadAnnotationsError
              ? 'error'
              : 'loaded'
        }
      />
      {showingNotes && (
        <NotesFilters
          noteKinds={noteKinds ?? []}
          catalogState={
            noteKindsPending ? 'loading' : noteKindsError ? 'error' : 'loaded'
          }
          kind={kind}
          tags={tags}
          tag={tag}
          view={view}
          tagCandidatesError={allTagOverviewsError || pendingTagOverviewsError}
          onViewChange={(value) => {
            void navigate({
              search: (previous) => ({ ...previous, view: value }),
            })
          }}
          onKindChange={(value) => {
            void navigate({
              search: (previous) => ({ ...previous, kind: value }),
            })
          }}
          onTagChange={(value) => {
            void navigate({
              search: (previous) => ({ ...previous, tag: value }),
            })
          }}
          onClear={() => {
            void navigate({
              search: (previous) => ({
                ...previous,
                kind: undefined,
                tag: undefined,
              }),
            })
          }}
        />
      )}
      <div
        id="notes-tabpanel"
        role="tabpanel"
        aria-labelledby={`notes-tab-${activeTab}`}
      >
        {showingNotes ? (
          <NotesList
            overviews={overviews ?? []}
            view={view}
            kind={kind}
            tag={tag}
            state={
              overviewsPending ? 'loading' : overviewsError ? 'error' : 'loaded'
            }
            noteKinds={noteKinds ?? []}
            noteKindCatalogState={
              noteKindsPending ? 'loading' : noteKindsError ? 'error' : 'loaded'
            }
            selectedItemIds={approval.state.selectedItemIds}
            selectedItems={approval.selectedItems}
            confirmedWarningDispositionVersionIds={
              confirmedWarningDispositionVersionIds
            }
            isApproving={approval.state.isApproving}
            hasApprovalError={approval.state.hasApprovalError}
            onToggleItem={onToggleReviewItem}
            onConfirmWarningDisposition={confirmWarningDisposition}
            onApproveWarningVersion={approveWarningVersion}
            onApproveSelected={approveSelected}
          />
        ) : (
          <AnnotationsList
            annotations={annotations ?? []}
            view={view}
            state={
              annotationsPending
                ? 'loading'
                : annotationsError
                  ? 'error'
                  : 'loaded'
            }
            kind={kind}
            tag={tag}
            selectedItemIds={approval.state.selectedItemIds}
            isApproving={approval.state.isApproving}
            hasApprovalError={approval.state.hasApprovalError}
            onViewChange={(value) => {
              void navigate({
                search: (previous) => ({ ...previous, view: value }),
              })
            }}
            onToggleItem={approval.actions.toggleItem}
            onToggleVisibleItems={approval.actions.toggleVisibleItems}
            onApproveSelected={approval.actions.approveSelected}
          />
        )}
      </div>
    </div>
  )
}
