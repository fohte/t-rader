import { Link } from '@tanstack/react-router'

import type { components } from '#lib/api/schema.gen'
import { formatRelative, isNewerThan } from '#lib/note-utils'

type Note = components['schemas']['Note']
type Annotation = components['schemas']['Annotation']

interface ArrivalsListProps {
  strategyId: string
  notes: Note[]
  annotations: Annotation[]
  since: number | null
}

interface Arrival {
  kind: 'note' | 'annotation'
  id: string
  noteId: string | null
  label: string
  updatedAt: string
}

function pickArrivals(
  notes: Note[],
  annotations: Annotation[],
  since: number | null,
): Arrival[] {
  const items: Arrival[] = []
  for (const n of notes) {
    if (isNewerThan(n.updated_at, since)) {
      items.push({
        kind: 'note',
        id: n.id,
        noteId: n.id,
        label: n.title,
        updatedAt: n.updated_at,
      })
    }
  }
  for (const a of annotations) {
    if (isNewerThan(a.updated_at, since)) {
      items.push({
        kind: 'annotation',
        id: a.id,
        noteId: a.linked_note_id ?? null,
        label: a.text,
        updatedAt: a.updated_at,
      })
    }
  }
  items.sort((a, b) => b.updatedAt.localeCompare(a.updatedAt))
  return items
}

const MAX_VISIBLE = 6

export function ArrivalsList({
  strategyId,
  notes,
  annotations,
  since,
}: ArrivalsListProps) {
  const arrivals = pickArrivals(notes, annotations, since)
  if (arrivals.length === 0) return null

  return (
    <section>
      <div className="mb-2 flex items-baseline gap-2 font-mono text-2xs uppercase tracking-wider text-muted-foreground">
        <span className="text-primary">&gt;</span>
        <span>新着</span>
        <span className="text-muted-foreground-strong">
          {since == null
            ? `${String(arrivals.length)} 件`
            : `前回開いてから ${String(arrivals.length)} 件`}
        </span>
      </div>
      <div className="border border-border bg-card">
        {arrivals.slice(0, MAX_VISIBLE).map((a) => {
          const target =
            a.kind === 'annotation'
              ? a.noteId != null
                ? {
                    to: '/strategies/$id/notes/$noteId' as const,
                    params: { id: strategyId, noteId: a.noteId },
                  }
                : {
                    to: '/strategies/$id/annotations/$annoId' as const,
                    params: { id: strategyId, annoId: a.id },
                  }
              : {
                  to: '/strategies/$id/notes/$noteId' as const,
                  params: { id: strategyId, noteId: a.noteId ?? a.id },
                }
          return (
            <Link
              key={`${a.kind}:${a.id}`}
              to={target.to}
              params={target.params}
              className="flex items-center gap-3 border-b border-border px-3.5 py-2.5 last:border-b-0 hover:bg-surface-strong"
            >
              <span className="w-23 flex-shrink-0 font-mono text-2xs uppercase tracking-wider text-muted-foreground">
                {a.kind === 'note' ? 'NOTE' : 'ANNOTATION'}
              </span>
              <span className="min-w-0 flex-1 truncate text-sm text-foreground">
                {a.label}
              </span>
              <span className="flex-shrink-0 font-mono text-2xs text-muted-foreground">
                {formatRelative(a.updatedAt)}
              </span>
            </Link>
          )
        })}
      </div>
    </section>
  )
}
