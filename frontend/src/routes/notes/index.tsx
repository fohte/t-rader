import { createFileRoute } from '@tanstack/react-router'

import { PendingNoteVersionsLink } from '#components/note-detail/pending-note-versions-link'
import { StrategyFilterSelect } from '#components/strategy-filter-select'
import { CreateNoteDialog } from '#components/strategy-home/create-note-dialog'
import { NoteTagFilterSelect } from '#components/strategy-home/note-tag-filter-select'
import { NotesList } from '#components/strategy-home/notes-list'
import { Skeleton } from '#components/ui/skeleton'
import { $api } from '#lib/api/client'

export const Route = createFileRoute('/notes/')({
  validateSearch: (
    search: Record<string, unknown>,
  ): { strategy_id?: string; tag?: string } => ({
    strategy_id:
      typeof search.strategy_id === 'string' ? search.strategy_id : undefined,
    tag: typeof search.tag === 'string' ? search.tag : undefined,
  }),
  component: NotesPage,
})

function NotesPage() {
  const { strategy_id, tag } = Route.useSearch()
  const navigate = Route.useNavigate()
  const { data: availableNotes } = $api.useQuery(
    'get',
    '/api/notes',
    { params: { query: { strategy_id } } },
    { enabled: tag != null },
  )
  const { data: notes, isPending } = $api.useQuery('get', '/api/notes', {
    params: { query: { strategy_id, tag } },
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
          <CreateNoteDialog strategyId={strategy_id} />
          <PendingNoteVersionsLink />
          <StrategyFilterSelect
            value={strategy_id}
            onChange={(v) => {
              void navigate({
                search: (prev) => ({ ...prev, strategy_id: v, tag: undefined }),
              })
            }}
          />
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
