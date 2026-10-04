import { createFileRoute } from '@tanstack/react-router'

import { PendingNoteVersionsLink } from '#components/note-detail/pending-note-versions-link'
import { CreateNoteDialog } from '#components/strategy-home/create-note-dialog'
import { NoteTagFilterSelect } from '#components/strategy-home/note-tag-filter-select'
import { NotesList } from '#components/strategy-home/notes-list'
import { Skeleton } from '#components/ui/skeleton'
import { $api } from '#lib/api/client'

export const Route = createFileRoute('/notes/')({
  validateSearch: (search: Record<string, unknown>): { tag?: string } => ({
    tag:
      typeof search.tag === 'string' && search.tag.length > 0
        ? search.tag
        : undefined,
  }),
  component: NotesPage,
})

function NotesPage() {
  const { tag } = Route.useSearch()
  const navigate = Route.useNavigate()
  const { data: availableNotes } = $api.useQuery(
    'get',
    '/api/notes',
    { params: { query: {} } },
    { enabled: tag != null },
  )
  const { data: notes, isPending } = $api.useQuery('get', '/api/notes', {
    params: { query: { tag } },
  })
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
          ノート
        </h1>
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
      </header>
      {isPending ? (
        <Skeleton className="h-40 w-full" />
      ) : (
        <NotesList notes={notes ?? []} />
      )}
    </div>
  )
}
