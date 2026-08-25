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
    <div className="border border-border bg-card">
      <div className="border-b border-border px-3.5 py-2 font-mono text-2xs uppercase tracking-wider text-muted-foreground">
        チャートのアノテーション · {items.length}
        {symbol != null && (
          <span className="ml-2 text-muted-foreground-strong">{symbol}</span>
        )}
      </div>
      {items.map((a) => {
        const isSelected = a.id === selectedAnnotationId
        return (
          <div
            key={a.id}
            className={`flex items-start gap-3 border-b border-border px-3.5 py-2.5 last:border-b-0 hover:bg-surface-strong ${
              isSelected ? 'bg-surface-strong' : ''
            }`}
          >
            <button
              type="button"
              onClick={() => onSelectAnnotation?.(a.id)}
              className={`mt-0.5 inline-grid h-5 min-w-7 flex-shrink-0 cursor-pointer place-items-center border px-1 font-mono text-2xs ${
                isSelected
                  ? 'border-primary bg-primary text-card'
                  : 'border-primary text-primary'
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
                <span className="line-clamp-2 block text-sm text-foreground">
                  {a.text}
                </span>
              </button>
              <div className="mt-1 flex items-center gap-2 font-mono text-2xs">
                <span className="border border-border bg-surface-strong px-1 text-2xs uppercase text-muted-foreground-strong">
                  {a.target_kind}
                </span>
                <StatusPill status={a.status} />
                <Link
                  to="/strategies/$id/annotations/$annoId"
                  params={{ id: strategyId, annoId: a.id }}
                  className="text-primary hover:underline"
                >
                  → 詳細
                </Link>
                {a.linked_note_id != null && (
                  <Link
                    to="/strategies/$id/notes/$noteId"
                    params={{ id: strategyId, noteId: a.linked_note_id }}
                    className="text-primary hover:underline"
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
