import { createFileRoute } from '@tanstack/react-router'

import { EmptyPlaceholder } from '@/components/strategy-shell/empty-placeholder'

export const Route = createFileRoute('/strategies/$id/notes/$noteId')({
  component: NoteDetailPage,
})

function NoteDetailPage() {
  const { noteId } = Route.useParams()
  return (
    <EmptyPlaceholder
      title={`ノート ${noteId}`}
      description="Markdown 本文 + コメントスレッド + status + 変更履歴を表示予定。"
    />
  )
}
