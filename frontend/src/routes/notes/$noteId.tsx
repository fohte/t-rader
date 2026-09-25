import { createFileRoute, Link } from '@tanstack/react-router'
import { useCallback, useRef, useState } from 'react'

import { CommentsPanel } from '#components/note-detail/comments-panel'
import { HistoricalVersionNotice } from '#components/note-detail/historical-version-notice'
import { HistoryPanel } from '#components/note-detail/history-panel'
import { NoteDocument } from '#components/note-detail/note-document'
import { NoteHeader } from '#components/note-detail/note-header'
import { NoteLinksPanelView } from '#components/note-detail/note-links-panel'
import { PredictionsPanel } from '#components/note-detail/predictions-panel'
import { ReviewPanel } from '#components/note-detail/review-panel'
import { openFloatingChat } from '#components/strategy-shell/floating-chat-store'
import { Skeleton } from '#components/ui/skeleton'
import { $api } from '#lib/api/client'

export const Route = createFileRoute('/notes/$noteId')({
  validateSearch: (
    search: Record<string, unknown>,
  ): { version_id?: string } => ({
    version_id:
      typeof search.version_id === 'string' ? search.version_id : undefined,
  }),
  component: NoteDetailPage,
})

function NoteDetailPage() {
  const { noteId } = Route.useParams()
  const { version_id } = Route.useSearch()
  const { data: note, isPending } = $api.useQuery('get', '/api/notes/{id}', {
    params: { path: { id: noteId }, query: { version_id } },
  })
  const {
    data: noteLinks,
    isPending: linksPending,
    isError: linksError,
  } = $api.useQuery(
    'get',
    '/api/notes/{id}/links',
    {
      params: {
        path: { id: noteId },
        query: { version_id: note?.version_id },
      },
    },
    { enabled: note != null },
  )
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
        <Skeleton className="h-120 w-full" />
      </div>
    )
  }

  if (note == null) {
    return (
      <div className="font-mono text-sm text-muted-foreground">
        ノートが見つかりませんでした。
      </div>
    )
  }

  return (
    <div className="space-y-4 font-sans text-foreground">
      {note.strategy_id != null && (
        <Link
          to="/strategies/$id/performance"
          params={{ id: note.strategy_id }}
          className="inline-flex items-center gap-1 font-mono text-xs text-muted-foreground hover:text-primary"
        >
          &lt; 戦略成績に戻る
        </Link>
      )}
      <div className="grid grid-cols-1 gap-5 lg:grid-cols-(--grid-cols-note-detail)">
        <article className="border border-border bg-card px-5 py-5">
          <NoteHeader note={note} strategyId={note.strategy_id ?? null} />
          <NoteDocument
            source={note.body_md}
            graphs={note.graphs_json}
            noteLinks={noteLinks?.outgoing}
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
          {note.is_current ? (
            <ReviewPanel noteId={note.id} status={note.status} />
          ) : (
            <HistoricalVersionNotice
              noteId={note.id}
              versionNo={note.version_no}
            />
          )}
          <PredictionsPanel noteId={note.id} />
          <NoteLinksPanelView
            outgoing={noteLinks?.outgoing ?? []}
            incoming={noteLinks?.incoming ?? []}
            isPending={linksPending}
            isError={linksError}
          />
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
