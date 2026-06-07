import { Link } from '@tanstack/react-router'

import { StatusPill } from '@/components/strategy-home/status-pill'
import type { components } from '@/lib/api/schema.gen'
import { formatRelative } from '@/lib/note-utils'

type Note = components['schemas']['Note']

interface NotesListProps {
  strategyId: string
  notes: Note[]
}

export function NotesList({ strategyId, notes }: NotesListProps) {
  return (
    <section className="border border-[color:var(--color-border-strategy)] bg-[color:var(--panel)]">
      <div className="flex items-baseline justify-between border-b border-[color:var(--color-hairline)] px-3.5 py-2">
        <h3 className="font-mono text-[12px] font-bold uppercase tracking-wider text-[color:var(--color-text-primary)]">
          ノート一覧
        </h3>
        <span className="font-mono text-[11px] text-[color:var(--color-text-tertiary)]">
          {notes.length}
        </span>
      </div>
      {notes.length === 0 ? (
        <div className="px-3.5 py-3 font-mono text-[12px] text-[color:var(--color-text-tertiary)]">
          —
        </div>
      ) : (
        <div>
          {notes.map((n) => (
            <Link
              key={n.id}
              to="/strategies/$id/notes/$noteId"
              params={{ id: strategyId, noteId: n.id }}
              className="flex flex-col gap-1 border-b border-[color:var(--color-hairline)] px-3.5 py-2.5 last:border-b-0 hover:bg-[color:var(--panel-inset)]"
            >
              <span className="line-clamp-2 text-[13px] text-[color:var(--color-text-primary)]">
                {n.title}
              </span>
              <span className="flex flex-wrap items-center gap-2 font-mono text-[11px]">
                {n.type_tag != null && n.type_tag !== '' && (
                  <span className="border border-[color:var(--color-border-strategy)] bg-[color:var(--panel-inset)] px-1 text-[10px] uppercase text-[color:var(--color-text-secondary)]">
                    {n.type_tag}
                  </span>
                )}
                <StatusPill status={n.status} />
                <span className="ml-auto text-[color:var(--color-text-tertiary)]">
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
