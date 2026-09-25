import { createFileRoute } from '@tanstack/react-router'

import { PendingNoteVersionsLink } from '#components/note-detail/pending-note-versions-link'
import { StrategyFilterSelect } from '#components/strategy-filter-select'
import { CreateNoteDialog } from '#components/strategy-home/create-note-dialog'
import { NotesList } from '#components/strategy-home/notes-list'
import { Skeleton } from '#components/ui/skeleton'
import { $api } from '#lib/api/client'

export const Route = createFileRoute('/notes/')({
  validateSearch: (
    search: Record<string, unknown>,
  ): { strategy_id?: string } => ({
    strategy_id:
      typeof search.strategy_id === 'string' ? search.strategy_id : undefined,
  }),
  component: NotesPage,
})

function NotesPage() {
  const { strategy_id } = Route.useSearch()
  const navigate = Route.useNavigate()
  const { data: notes, isPending } = $api.useQuery('get', '/api/notes', {
    params: { query: { strategy_id } },
  })

  return (
    <div className="space-y-4 font-sans text-foreground">
      <header className="flex items-center justify-between gap-3">
        <h1 className="text-2xl font-bold leading-tight tracking-tight">
          ノート
        </h1>
        <div className="flex items-center gap-2">
          <CreateNoteDialog strategyId={strategy_id} />
          <PendingNoteVersionsLink />
          <StrategyFilterSelect
            value={strategy_id}
            onChange={(v) => {
              void navigate({ search: (prev) => ({ ...prev, strategy_id: v }) })
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
