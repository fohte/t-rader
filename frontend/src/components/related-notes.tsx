import { Link } from '@tanstack/react-router'

import { NoteOverviewStatus } from '#components/note-overview-status'
import { Skeleton } from '#components/ui/skeleton'
import type { components } from '#lib/api/schema.gen'
import { formatRelative, sortNoteOverviewsNewestFirst } from '#lib/note-utils'

type NoteOverview = components['schemas']['NoteOverviewResponse']

interface RelatedNotesProps {
  notes: NoteOverview[] | null
  isError: boolean
}

export function RelatedNotes({ notes, isError }: RelatedNotesProps) {
  const sortedNotes = notes == null ? [] : sortNoteOverviewsNewestFirst(notes)

  return (
    <section className="space-y-2" aria-labelledby="related-notes-title">
      <div className="flex items-baseline justify-between gap-3">
        <h2
          id="related-notes-title"
          className="font-mono text-2xs uppercase tracking-wider text-muted-foreground"
        >
          <span className="mr-2 text-muted-foreground">&gt;</span>
          関連ノート
        </h2>
        <span className="shrink-0 font-mono text-2xs text-muted-foreground">
          {notes == null ? '—' : `${notes.length.toString()} 件`}
        </span>
      </div>
      <div className="divide-y divide-border border border-border bg-card">
        {isError ? (
          <p className="px-4 py-5 font-mono text-xs text-muted-foreground">
            ノートを読み込めませんでした。
          </p>
        ) : notes == null ? (
          <div className="space-y-2 p-3">
            <Skeleton className="h-10 w-full" />
            <Skeleton className="h-10 w-full" />
          </div>
        ) : sortedNotes.length === 0 ? (
          <p className="px-4 py-7 text-center font-mono text-xs text-muted-foreground">
            関連するノートはありません。
          </p>
        ) : (
          sortedNotes.map((overview) => {
            const note = overview.representative

            return (
              <Link
                key={note.id}
                to="/notes/$noteId"
                params={{ noteId: note.id }}
                className="flex min-w-0 flex-col gap-1 px-3.5 py-2.5 hover:bg-surface-strong"
              >
                <span className="truncate text-sm text-foreground">
                  {note.title}
                </span>
                <span className="font-mono text-2xs text-muted-foreground">
                  {note.created_by_kind === 'human' ? '自分' : 'エージェント'} ·{' '}
                  {formatRelative(note.updated_at)}
                </span>
                <NoteOverviewStatus overview={overview} />
              </Link>
            )
          })
        )}
      </div>
    </section>
  )
}
