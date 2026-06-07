import { TriggerTag } from '@/components/strategy-home/trigger-tag'
import { RefChip } from '@/components/strategy-shell/ref-chip'
import type { components } from '@/lib/api/schema.gen'
import { extractRefs, formatRelative } from '@/lib/note-utils'

type Note = components['schemas']['Note']

interface NoteHeaderProps {
  note: Note
  strategyId: string
}

export function NoteHeader({ note, strategyId }: NoteHeaderProps) {
  const refs = extractRefs(note)
  const isLLM = note.created_by_kind === 'llm'

  return (
    <header className="mb-5 border-b border-[color:var(--color-hairline)] pb-4">
      <div className="mb-3 border border-[color:var(--color-border-strategy)] bg-[color:var(--panel-inset)] px-3 py-2 font-mono text-[11.5px] leading-relaxed text-[color:var(--color-text-secondary)]">
        <span className="text-[color:var(--color-text-tertiary)]">type:</span>{' '}
        <span className="text-[color:var(--color-text-primary)]">
          {note.type_tag ?? '—'}
        </span>{' '}
        <span className="text-[color:var(--color-text-tertiary)]">status:</span>{' '}
        <span className="text-[color:var(--color-text-primary)]">
          {note.status}
        </span>{' '}
        <span className="text-[color:var(--color-text-tertiary)]">
          strategy:
        </span>{' '}
        <span className="text-[color:var(--color-text-primary)]">
          {strategyId}
        </span>
        {refs.length > 0 && (
          <>
            <br />
            <span className="text-[color:var(--color-text-tertiary)]">
              refs:
            </span>{' '}
            <span className="inline-flex flex-wrap items-center gap-1.5 align-baseline">
              {refs.map((r) => (
                <RefChip key={r} token={r} />
              ))}
            </span>
          </>
        )}
      </div>
      <h1 className="mb-2 text-[26px] font-bold leading-tight tracking-tight text-[color:var(--color-text-primary)]">
        {note.title}
      </h1>
      <div className="flex flex-wrap items-center gap-x-3 gap-y-1.5 font-mono text-[11px] text-[color:var(--color-text-tertiary)]">
        <span className="inline-flex items-center gap-1.5">
          <span
            className={`font-bold ${isLLM ? 'text-[color:var(--color-accent-strategy)]' : 'text-[color:var(--color-text-primary)]'}`}
          >
            &gt;
          </span>
          {isLLM ? 'analyst が作成' : 'ユーザー が作成'}
        </span>
        <span>·</span>
        <TriggerTag trigger={note.trigger} label={note.trigger_label} />
        <span>·</span>
        <span>
          {formatRelative(note.created_at)} 作成 /{' '}
          {formatRelative(note.updated_at)} 更新
        </span>
      </div>
    </header>
  )
}
