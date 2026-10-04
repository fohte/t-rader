import { Link } from '@tanstack/react-router'

import { StatusPill } from '#components/strategy-home/status-pill'
import type { components } from '#lib/api/schema.gen'
import { buildSnippet, formatRelative } from '#lib/note-utils'

type Annotation = components['schemas']['Annotation']

export function AnnotationsList({
  annotations,
}: {
  annotations: Annotation[]
}) {
  return (
    <section className="border border-border bg-card">
      <div className="flex items-baseline justify-between border-b border-border px-3.5 py-2">
        <h2 className="font-mono text-xs font-bold uppercase tracking-wider text-foreground">
          一覧
        </h2>
        <span className="font-mono text-2xs text-muted-foreground">
          {annotations.length}
        </span>
      </div>
      {annotations.length === 0 ? (
        <div className="px-3.5 py-3 font-mono text-xs text-muted-foreground">
          —
        </div>
      ) : (
        <div>
          {annotations.map((annotation) => (
            <Link
              key={annotation.id}
              to="/annotations/$annoId"
              params={{ annoId: annotation.id }}
              className="flex flex-col gap-1 border-b border-border px-3.5 py-2.5 last:border-b-0 hover:bg-surface-strong"
            >
              <span className="line-clamp-2 text-sm text-foreground">
                {buildSnippet(annotation.text)}
              </span>
              <span className="flex flex-wrap items-center gap-2 font-mono text-2xs">
                <span className="border border-border bg-surface-strong px-1 text-2xs uppercase text-muted-foreground-strong">
                  {annotation.target_symbol}
                </span>
                <StatusPill status={annotation.status} />
                <span className="ml-auto text-muted-foreground">
                  {formatRelative(annotation.updated_at)}
                </span>
              </span>
            </Link>
          ))}
        </div>
      )}
    </section>
  )
}
