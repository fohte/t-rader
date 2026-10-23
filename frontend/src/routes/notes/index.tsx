import { useQueryClient } from '@tanstack/react-query'
import { createFileRoute } from '@tanstack/react-router'
import { useEffect } from 'react'

import { AnnotationsList } from '#components/annotations/annotations-list'
import { AnnotationsListSkeleton } from '#components/annotations/annotations-list-skeleton'
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
  const { data: allTagOverviews } = $api.useQuery(
    'get',
    '/api/notes/overview',
    { params: { query: { view: 'all' } } },
    { enabled: showingNotes },
  )
  const { data: pendingTagOverviews } = $api.useQuery(
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
  const { data: annotations, isPending: annotationsPending } = $api.useQuery(
    'get',
    '/api/annotations',
    undefined,
    { enabled: !showingNotes },
  )
  const approveVersion = $api.useMutation(
    'post',
    '/api/notes/{id}/versions/{n}/approve',
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
  const reviewData = createReviewData(overviews)

  function invalidateReviewData(): void {
    for (const queryKey of getReviewQueueQueryKeys()) {
      void queryClient.invalidateQueries({ queryKey })
    }
    void queryClient.invalidateQueries({
      queryKey: $api.queryOptions('get', '/api/notes').queryKey,
    })
  }

  function approveItem(
    item: ReviewQueueItem,
    callbacks: ReviewApprovalCallbacks,
  ): void {
    if (item.kind !== 'note') return
    approveVersion.mutate(
      {
        params: { path: { id: item.noteId, n: item.versionNo } },
        body: {},
      },
      callbacks,
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
  }, [activeTab, clearSelection, kind, tag, view])

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
      <NotesTabs activeTab={activeTab} />
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
            isApproving={approval.state.isApproving}
            hasApprovalError={approval.state.hasApprovalError}
            onToggleItem={approval.actions.toggleItem}
            onApproveSelected={approval.actions.approveSelected}
          />
        ) : annotationsPending ? (
          <AnnotationsListSkeleton />
        ) : (
          <AnnotationsList annotations={annotations ?? []} />
        )}
      </div>
    </div>
  )
}
