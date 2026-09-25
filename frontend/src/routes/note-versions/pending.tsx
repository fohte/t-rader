import { createFileRoute } from '@tanstack/react-router'

import { PendingNoteVersionsView } from '#components/note-detail/pending-note-versions-view'
import { $api } from '#lib/api/client'

export const Route = createFileRoute('/note-versions/pending')({
  component: PendingNoteVersionsPage,
})

function PendingNoteVersionsPage() {
  const {
    data: versions,
    isPending,
    isError,
  } = $api.useQuery('get', '/api/note-versions/pending')

  return (
    <PendingNoteVersionsView
      versions={versions ?? []}
      isPending={isPending}
      hasError={isError}
    />
  )
}
