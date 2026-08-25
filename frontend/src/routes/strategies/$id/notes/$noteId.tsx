import { createFileRoute, Link } from '@tanstack/react-router'
import { useCallback, useRef, useState } from 'react'

import { CommentsPanel } from '#components/note-detail/comments-panel'
import { HistoryPanel } from '#components/note-detail/history-panel'
import { NoteDocument } from '#components/note-detail/note-document'
import { NoteHeader } from '#components/note-detail/note-header'
import { ReviewPanel } from '#components/note-detail/review-panel'
import { openFloatingChat } from '#components/strategy-shell/floating-chat-store'
import { Skeleton } from '#components/ui/skeleton'
import { $api } from '#lib/api/client'

export const Route = createFileRoute('/strategies/$id/notes/$noteId')({
  component: NoteDetailPage,
})

function NoteDetailPage() {
  const { id, noteId } = Route.useParams()
  const { data: note, isPending } = $api.useQuery('get', '/api/notes/{id}', {
    params: { path: { id: noteId } },
  })
  const [pendingQuote, setPendingQuote] = useState<string | null>(null)
  const bodyRef = useRef<HTMLDivElement>(null)
  const onConsumeQuote = useCallback(() => {
    setPendingQuote(null)
  }, [])
  const onQuoteSelection = useCallback((text: string) => {
    setPendingQuote(text)
  }, [])

  if (isPending) {
    return (
      <div className="space-y-4">
        <Skeleton className="h-6 w-32" />
        <Skeleton className="h-10 w-2/3" />
        <Skeleton className="h-[480px] w-full" />
      </div>
    )
  }

  if (note == null) {
    return (
      <div className="font-mono text-[13px] text-muted-foreground">
        ノートが見つかりませんでした。
      </div>
    )
  }

  return (
    <div className="space-y-4 font-sans text-foreground">
      <Link
        to="/strategies/$id"
        params={{ id }}
        className="inline-flex items-center gap-1 font-mono text-xs text-muted-foreground hover:text-primary"
      >
        &lt; 戦略ホームに戻る
      </Link>
      <div className="grid grid-cols-1 gap-5 lg:grid-cols-[minmax(0,1fr)_340px]">
        <article className="border border-border bg-card px-5 py-5">
          <NoteHeader note={note} strategyId={id} />
          <NoteDocument
            source={note.body_md}
            graphs={note.graphs_json}
            onQuoteSelection={onQuoteSelection}
            bodyRef={bodyRef}
          />
          <div className="mt-5 flex flex-wrap items-center gap-2 border-t border-border pt-4 font-mono text-2xs text-muted-foreground">
            <span>このノートについて</span>
            <button
              type="button"
              onClick={() => {
                openFloatingChat(`「${note.title}」について補足して`)
              }}
              className="inline-flex items-center gap-1 border border-border px-2 py-0.5 text-muted-foreground-strong hover:border-primary hover:text-primary"
            >
              <span className="font-bold text-primary">&gt;_</span>
              アナリストに聞く
            </button>
          </div>
        </article>
        <aside className="space-y-4">
          <ReviewPanel noteId={note.id} strategyId={id} status={note.status} />
          <CommentsPanel
            noteId={note.id}
            pendingQuote={pendingQuote}
            onConsumeQuote={onConsumeQuote}
          />
          <HistoryPanel noteId={note.id} />
        </aside>
      </div>
    </div>
  )
}
