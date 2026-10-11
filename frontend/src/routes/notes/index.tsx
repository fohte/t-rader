import { createFileRoute } from '@tanstack/react-router'

import { AnnotationsList } from '#components/annotations/annotations-list'
import { AnnotationsListSkeleton } from '#components/annotations/annotations-list-skeleton'
import { PendingNoteVersionsLink } from '#components/note-detail/pending-note-versions-link'
import {
  type NotesTab,
  NotesTabs,
  notesTabSearch,
} from '#components/notes/notes-tabs'
import { CreateNoteDialog } from '#components/strategy-home/create-note-dialog'
import { NoteTagFilterSelect } from '#components/strategy-home/note-tag-filter-select'
import { NotesList } from '#components/strategy-home/notes-list'
import { Skeleton } from '#components/ui/skeleton'
import { $api } from '#lib/api/client'

export const Route = createFileRoute('/notes/')({
  validateSearch: (
    search: Record<string, unknown>,
  ): { tag?: string; tab?: NotesTab } => ({
    tag:
      typeof search.tag === 'string' && search.tag.length > 0
        ? search.tag
        : undefined,
    ...notesTabSearch(search.tab === 'annotations' ? 'annotations' : 'notes'),
  }),
  component: NotesPage,
})

function NotesPage() {
  const { tag, tab } = Route.useSearch()
  const navigate = Route.useNavigate()
  const activeTab = tab ?? 'notes'
  const showingNotes = activeTab === 'notes'
  const { data: availableNotes } = $api.useQuery(
    'get',
    '/api/notes',
    { params: { query: {} } },
    { enabled: showingNotes && tag != null },
  )
  const { data: notes, isPending: notesPending } = $api.useQuery(
    'get',
    '/api/notes',
    { params: { query: { tag } } },
    { enabled: showingNotes },
  )
  const { data: annotations, isPending: annotationsPending } = $api.useQuery(
    'get',
    '/api/annotations',
    undefined,
    { enabled: !showingNotes },
  )
  const filterNotes = tag == null ? (notes ?? []) : (availableNotes ?? [])
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
            <PendingNoteVersionsLink />
            <NoteTagFilterSelect
              tags={tags}
              value={tag}
              onChange={(v) => {
                void navigate({ search: (prev) => ({ ...prev, tag: v }) })
              }}
            />
          </div>
        )}
      </header>
      <NotesTabs activeTab={activeTab} />
      <div
        id="notes-tabpanel"
        role="tabpanel"
        aria-labelledby={`notes-tab-${activeTab}`}
      >
        {showingNotes ? (
          notesPending ? (
            <Skeleton className="h-40 w-full" />
          ) : (
            <NotesList notes={notes ?? []} />
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
