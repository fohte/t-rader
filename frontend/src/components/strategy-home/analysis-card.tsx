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
      className={`flex flex-col gap-2.5 border bg-[color:var(--panel)] p-3.5 transition-colors hover:border-[color:var(--color-text-tertiary)] ${
        unread
          ? 'border-[color:var(--color-accent-strategy)]/50'
          : 'border-[color:var(--color-border-strategy)]'
      }`}
    >
      <div className="flex items-center gap-2 text-[10px]">
        {note.type_tag != null && note.type_tag !== '' && (
          <span className="border border-[color:var(--color-border-strategy)] bg-[color:var(--panel-inset)] px-1.5 py-px font-mono uppercase tracking-wider text-[color:var(--color-text-secondary)]">
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
        <h4 className="text-[15px] font-bold leading-tight text-[color:var(--color-text-primary)] hover:text-[color:var(--color-accent-strategy)]">
          {note.title}
        </h4>
      </Link>
      {snippet !== '' && (
        <p className="line-clamp-3 text-[13px] leading-relaxed text-[color:var(--color-text-secondary)]">
          {snippet}
        </p>
      )}
      <div className="flex flex-wrap items-center gap-2 border-t border-[color:var(--color-hairline)] pt-2.5 font-mono text-[11px]">
        <span className="text-[color:var(--color-text-tertiary)]">
          <span className="text-[color:var(--color-accent-strategy)]">
            &gt;{' '}
          </span>
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
          className="inline-flex items-center gap-1 border border-[color:var(--color-border-strategy)] bg-[color:var(--color-bg-secondary)] px-2 py-0.5 text-[color:var(--color-text-secondary)] hover:border-[color:var(--color-accent-strategy)] hover:text-[color:var(--color-accent-strategy)]"
        >
          <span className="font-bold text-[color:var(--color-accent-strategy)]">
            &gt;_
          </span>
          聞く
        </button>
        <span className="text-[color:var(--color-text-tertiary)]">
          {formatRelative(note.updated_at)}
        </span>
      </div>
    </div>
  )
}
