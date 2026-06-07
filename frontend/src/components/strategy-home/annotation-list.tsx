import { Link } from '@tanstack/react-router'

import { StatusPill } from '@/components/strategy-home/status-pill'
import type { components } from '@/lib/api/schema.gen'

type Annotation = components['schemas']['Annotation']

interface AnnotationListProps {
  strategyId: string
  annotations: Annotation[]
}

export function AnnotationList({
  strategyId,
  annotations,
}: AnnotationListProps) {
  if (annotations.length === 0) return null
  return (
    <div className="border border-[color:var(--color-border-strategy)] bg-[color:var(--panel)]">
      <div className="border-b border-[color:var(--color-hairline)] px-3.5 py-2 font-mono text-[11px] uppercase tracking-wider text-[color:var(--color-text-tertiary)]">
        チャートのアノテーション · {annotations.length}
      </div>
      {annotations.map((a, i) => {
        const target =
          a.linked_note_id != null
            ? {
                to: '/strategies/$id/notes/$noteId' as const,
                params: { id: strategyId, noteId: a.linked_note_id },
              }
            : {
                to: '/strategies/$id/annotations/$annoId' as const,
                params: { id: strategyId, annoId: a.id },
              }
        return (
          <Link
            key={a.id}
            to={target.to}
            params={target.params}
            className="flex items-start gap-3 border-b border-[color:var(--color-hairline)] px-3.5 py-2.5 last:border-b-0 hover:bg-[color:var(--panel-inset)]"
          >
            <span className="mt-0.5 inline-grid h-5 min-w-[28px] flex-shrink-0 place-items-center border border-[color:var(--color-accent-strategy)] px-1 font-mono text-[10px] text-[color:var(--color-accent-strategy)]">
              A{i + 1}
            </span>
            <div className="min-w-0 flex-1">
              <p className="line-clamp-2 text-[13px] text-[color:var(--color-text-primary)]">
                {a.text}
              </p>
              <div className="mt-1 flex items-center gap-2 font-mono text-[11px]">
                <span className="border border-[color:var(--color-border-strategy)] bg-[color:var(--panel-inset)] px-1 text-[10px] uppercase text-[color:var(--color-text-secondary)]">
                  {a.target_kind}
                </span>
                <StatusPill status={a.status} />
                <span className="text-[color:var(--color-accent-strategy)]">
                  → 開く
                </span>
              </div>
            </div>
          </Link>
        )
      })}
    </div>
  )
}
