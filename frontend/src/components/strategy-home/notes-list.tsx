import { Link } from '@tanstack/react-router'

import { StatusPill } from '#components/strategy-home/status-pill'
import type { components } from '#lib/api/schema.gen'
import { formatRelative } from '#lib/note-utils'

type Note = components['schemas']['Note']

interface NotesListProps {
  strategyId: string
  notes: Note[]
}

export function NotesList({ strategyId, notes }: NotesListProps) {
  return (
    <section className="border border-border bg-card">
      <div className="flex items-baseline justify-between border-b border-border px-3.5 py-2">
        <h3 className="font-mono text-xs font-bold uppercase tracking-wider text-foreground">
          ノート一覧
        </h3>
        <span className="font-mono text-2xs text-muted-foreground">
          {notes.length}
        </span>
      </div>
      {notes.length === 0 ? (
        <div className="px-3.5 py-3 font-mono text-xs text-muted-foreground">
          —
        </div>
      ) : (
        <div>
          {notes.map((n) => (
            <Link
              key={n.id}
              to="/strategies/$id/notes/$noteId"
              params={{ id: strategyId, noteId: n.id }}
              className="flex flex-col gap-1 border-b border-border px-3.5 py-2.5 last:border-b-0 hover:bg-surface-strong"
            >
              <span className="line-clamp-2 text-sm text-foreground">
                {n.title}
              </span>
              <span className="flex flex-wrap items-center gap-2 font-mono text-2xs">
                {n.type_tag != null && n.type_tag !== '' && (
                  <span className="border border-border bg-surface-strong px-1 text-2xs uppercase text-muted-foreground-strong">
                    {n.type_tag}
                  </span>
                )}
                <StatusPill status={n.status} />
                <span className="ml-auto text-muted-foreground">
                  {formatRelative(n.updated_at)}
                </span>
              </span>
            </Link>
          ))}
        </div>
      )}
    </section>
  )
}
