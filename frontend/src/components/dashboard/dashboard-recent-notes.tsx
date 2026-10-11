import { Link } from '@tanstack/react-router'

import { Skeleton } from '#components/ui/skeleton'
import type { components } from '#lib/api/schema.gen'
import { formatRelative } from '#lib/note-utils'

type Note = components['schemas']['Note']

export function DashboardRecentNotes({ notes }: { notes: Note[] | null }) {
  const recentNotes =
    notes == null
      ? []
      : [...notes]
          .sort((left, right) =>
            right.updated_at.localeCompare(left.updated_at),
          )
          .slice(0, 3)

  return (
    <section className="space-y-2" aria-labelledby="dashboard-notes-title">
      <div className="flex items-baseline justify-between gap-3">
        <h2
          id="dashboard-notes-title"
          className="font-mono text-2xs uppercase tracking-wider text-muted-foreground"
        >
          <span className="mr-2 text-primary">&gt;</span>
          最近のノート
        </h2>
        <Link
          to="/notes"
          className="shrink-0 font-mono text-2xs text-primary hover:underline"
        >
          ノート一覧 →
        </Link>
      </div>
      <div className="divide-y divide-border border border-border bg-card">
        {notes == null ? (
          <div className="space-y-2 p-3">
            <Skeleton className="h-10 w-full" />
            <Skeleton className="h-10 w-full" />
          </div>
        ) : recentNotes.length === 0 ? (
          <p className="px-4 py-7 text-center font-mono text-xs text-muted-foreground">
            ノートはまだありません。
          </p>
        ) : (
          recentNotes.map((note) => (
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
            </Link>
          ))
        )}
      </div>
    </section>
  )
}
