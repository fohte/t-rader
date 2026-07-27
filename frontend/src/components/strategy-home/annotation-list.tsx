import { Link } from '@tanstack/react-router'

import { StatusPill } from '#components/strategy-home/status-pill'
import type { NumberedAnnotation } from '#lib/annotation-utils'

interface AnnotationListProps {
  strategyId: string
  /** 採番済みのアノテーション一覧。チャートと共通の (annotations, symbol) から派生していること */
  items: NumberedAnnotation[]
  /** 表示中の銘柄。ラベル表示にのみ使用 */
  symbol?: string | null
  selectedAnnotationId?: string | null
  onSelectAnnotation?: (id: string) => void
}

export function AnnotationList({
  strategyId,
  items,
  symbol,
  selectedAnnotationId,
  onSelectAnnotation,
}: AnnotationListProps) {
  if (items.length === 0) return null
  return (
    <div className="border border-[color:var(--color-border-strategy)] bg-[color:var(--panel)]">
      <div className="border-b border-[color:var(--color-hairline)] px-3.5 py-2 font-mono text-[11px] uppercase tracking-wider text-[color:var(--color-text-tertiary)]">
        チャートのアノテーション · {items.length}
        {symbol != null && (
          <span className="ml-2 text-[color:var(--color-text-secondary)]">
            {symbol}
          </span>
        )}
      </div>
      {items.map((a) => {
        const isSelected = a.id === selectedAnnotationId
        return (
          <div
            key={a.id}
            className={`flex items-start gap-3 border-b border-[color:var(--color-hairline)] px-3.5 py-2.5 last:border-b-0 hover:bg-[color:var(--panel-inset)] ${
              isSelected ? 'bg-[color:var(--panel-inset)]' : ''
            }`}
          >
            <button
              type="button"
              onClick={() => onSelectAnnotation?.(a.id)}
              className={`mt-0.5 inline-grid h-5 min-w-[28px] flex-shrink-0 cursor-pointer place-items-center border px-1 font-mono text-[10px] ${
                isSelected
                  ? 'border-[color:var(--color-accent-strategy)] bg-[color:var(--color-accent-strategy)] text-[color:var(--panel)]'
                  : 'border-[color:var(--color-accent-strategy)] text-[color:var(--color-accent-strategy)]'
              }`}
              aria-label={`アノテーション ${a.label} を選択`}
            >
              {a.label}
            </button>
            <div className="min-w-0 flex-1">
              <button
                type="button"
                onClick={() => onSelectAnnotation?.(a.id)}
                className="w-full cursor-pointer text-left"
              >
                <span className="line-clamp-2 block text-[13px] text-[color:var(--color-text-primary)]">
                  {a.text}
                </span>
              </button>
              <div className="mt-1 flex items-center gap-2 font-mono text-[11px]">
                <span className="border border-[color:var(--color-border-strategy)] bg-[color:var(--panel-inset)] px-1 text-[10px] uppercase text-[color:var(--color-text-secondary)]">
                  {a.target_kind}
                </span>
                <StatusPill status={a.status} />
                <Link
                  to="/strategies/$id/annotations/$annoId"
                  params={{ id: strategyId, annoId: a.id }}
                  className="text-[color:var(--color-accent-strategy)] hover:underline"
                >
                  → 詳細
                </Link>
                {a.linked_note_id != null && (
                  <Link
                    to="/strategies/$id/notes/$noteId"
                    params={{ id: strategyId, noteId: a.linked_note_id }}
                    className="text-[color:var(--color-accent-strategy)] hover:underline"
                  >
                    → note を開く
                  </Link>
                )}
              </div>
            </div>
          </div>
        )
      })}
    </div>
  )
}
