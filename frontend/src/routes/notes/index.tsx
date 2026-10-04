import { createFileRoute } from '@tanstack/react-router'

import { PendingNoteVersionsLink } from '#components/note-detail/pending-note-versions-link'
import { CreateNoteDialog } from '#components/strategy-home/create-note-dialog'
import { NotesList } from '#components/strategy-home/notes-list'
import { Skeleton } from '#components/ui/skeleton'
import { $api } from '#lib/api/client'

export const Route = createFileRoute('/notes/')({
  component: NotesPage,
})

function NotesPage() {
  const { data: notes, isPending } = $api.useQuery('get', '/api/notes')

  return (
    <div className="space-y-4 font-sans text-foreground">
      <header className="flex items-center justify-between gap-3">
        <h1 className="text-2xl font-bold leading-tight tracking-tight">
          ノート
        </h1>
        <div className="flex items-center gap-2">
          <CreateNoteDialog />
          <PendingNoteVersionsLink />
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
