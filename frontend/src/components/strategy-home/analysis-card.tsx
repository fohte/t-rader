import { Link } from '@tanstack/react-router'

import { StatusPill } from '#components/strategy-home/status-pill'
import { TriggerTag } from '#components/strategy-home/trigger-tag'
import { openFloatingChat } from '#components/strategy-shell/floating-chat-store'
import { RefChip } from '#components/strategy-shell/ref-chip'
import type { components } from '#lib/api/schema.gen'
import { buildSnippet, extractRefs, formatRelative } from '#lib/note-utils'

type Note = components['schemas']['Note']

interface AnalysisCardProps {
  note: Note
  strategyId: string
}

export function AnalysisCard({ note, strategyId }: AnalysisCardProps) {
  const refs = extractRefs(note).slice(0, 4)
  const snippet = buildSnippet(note.body_md)
  const unread = note.status === 'unread'

  return (
    <div
      className={`flex flex-col gap-2.5 border bg-card p-3.5 transition-colors hover:border-muted-foreground ${
        unread ? 'border-primary/50' : 'border-border'
      }`}
    >
      <div className="flex items-center gap-2 text-[10px]">
        {note.type_tag != null && note.type_tag !== '' && (
          <span className="border border-border bg-surface-strong px-1.5 py-px font-mono uppercase tracking-wider text-muted-foreground-strong">
            {note.type_tag}
          </span>
        )}
        <StatusPill status={note.status} />
        <span className="flex-1" />
        <TriggerTag trigger={note.trigger} label={note.trigger_label} />
      </div>
      <Link
        to="/strategies/$id/notes/$noteId"
        params={{ id: strategyId, noteId: note.id }}
        className="block"
      >
        <h4 className="text-[15px] font-bold leading-tight text-foreground hover:text-primary">
          {note.title}
        </h4>
      </Link>
      {snippet !== '' && (
        <p className="line-clamp-3 text-[13px] leading-relaxed text-muted-foreground-strong">
          {snippet}
        </p>
      )}
      <div className="flex flex-wrap items-center gap-2 border-t border-border pt-2.5 font-mono text-2xs">
        <span className="text-muted-foreground">
          <span className="text-primary">&gt; </span>
          {note.created_by_kind === 'llm' ? 'analyst' : note.created_by_kind}
        </span>
        {refs.length > 0 && (
          <span className="flex flex-wrap items-center gap-1.5">
            {refs.map((token) => (
              <RefChip key={token} token={token} pill />
            ))}
          </span>
        )}
        <span className="flex-1" />
        <button
          type="button"
          onClick={(e) => {
            e.stopPropagation()
            openFloatingChat(`「${note.title}」について補足して`)
          }}
          title="このノートについてアナリストに聞く"
          className="inline-flex items-center gap-1 border border-border bg-bg-secondary px-2 py-0.5 text-muted-foreground-strong hover:border-primary hover:text-primary"
        >
          <span className="font-bold text-primary">&gt;_</span>
          聞く
        </button>
        <span className="text-muted-foreground">
          {formatRelative(note.updated_at)}
        </span>
      </div>
    </div>
  )
}
