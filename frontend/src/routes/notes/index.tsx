import { createFileRoute } from '@tanstack/react-router'

import { AnnotationsList } from '#components/annotations/annotations-list'
import { AnnotationsListSkeleton } from '#components/annotations/annotations-list-skeleton'
import { ReviewLink } from '#components/note-detail/review-link'
import {
  type NotesTab,
  NotesTabs,
  notesTabSearch,
} from '#components/notes/notes-tabs'
import { CreateNoteDialog } from '#components/strategy-home/create-note-dialog'
import { NotesFilters } from '#components/strategy-home/notes-filters'
import { NotesList } from '#components/strategy-home/notes-list'
import { Skeleton } from '#components/ui/skeleton'
import { $api } from '#lib/api/client'

export const Route = createFileRoute('/notes/')({
  validateSearch: (
    search: Record<string, unknown>,
  ): { kind?: string; tag?: string; tab?: NotesTab } => ({
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

function NotesPage() {
  const { kind, tag, tab } = Route.useSearch()
  const navigate = Route.useNavigate()
  const activeTab = tab ?? 'notes'
  const showingNotes = activeTab === 'notes'
  const {
    data: noteKinds,
    isPending: noteKindsPending,
    isError: noteKindsError,
  } = $api.useQuery('get', '/api/note-kinds', undefined, {
    enabled: showingNotes,
  })
  const { data: availableNotes } = $api.useQuery(
    'get',
    '/api/notes',
    { params: { query: {} } },
    { enabled: showingNotes && (kind != null || tag != null) },
  )
  const { data: notes, isPending: notesPending } = $api.useQuery(
    'get',
    '/api/notes',
    { params: { query: { kind, tag } } },
    { enabled: showingNotes },
  )
  const { data: annotations, isPending: annotationsPending } = $api.useQuery(
    'get',
    '/api/annotations',
    undefined,
    { enabled: !showingNotes },
  )
  const filterNotes =
    kind == null && tag == null ? (notes ?? []) : (availableNotes ?? [])
  const tagSet = new Set(filterNotes.flatMap((note) => note.tags))
  if (tag != null) tagSet.add(tag)
  const tags = [...tagSet].sort((left, right) =>
    left.localeCompare(right, 'ja'),
  )

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
          onKindChange={(value) => {
            void navigate({ search: (prev) => ({ ...prev, kind: value }) })
          }}
          onTagChange={(value) => {
            void navigate({ search: (prev) => ({ ...prev, tag: value }) })
          }}
          onClear={() => {
            void navigate({
              search: (prev) => ({ ...prev, kind: undefined, tag: undefined }),
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
          notesPending ? (
            <Skeleton className="h-40 w-full" />
          ) : (
            <NotesList
              notes={notes ?? []}
              noteKinds={noteKinds ?? []}
              noteKindCatalogState={
                noteKindsPending
                  ? 'loading'
                  : noteKindsError
                    ? 'error'
                    : 'loaded'
              }
            />
          )
        ) : annotationsPending ? (
          <AnnotationsListSkeleton />
        ) : (
          <AnnotationsList annotations={annotations ?? []} />
        )}
      </div>
    </div>
  )
}
