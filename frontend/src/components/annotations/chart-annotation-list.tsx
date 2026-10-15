import type { CSSProperties } from 'react'

import type { ChartAnnotation } from '#lib/annotation-chart-utils'
import { buildSnippet } from '#lib/note-utils'
import { cn } from '#lib/utils'

interface ChartAnnotationListProps {
  annotations: ChartAnnotation[]
  selectedAnnotationId: string | null
  isLoading?: boolean
  isDeleting?: boolean
  deleteError?: string | null
  className?: string
  style?: CSSProperties
  onSelectAnnotation: (id: string) => void
  onDeleteAnnotation?: (id: string) => void
}

const dateFormatter = new Intl.DateTimeFormat('ja-JP', {
  month: 'short',
  day: 'numeric',
  timeZone: 'Asia/Tokyo',
})

export function ChartAnnotationList({
  annotations,
  selectedAnnotationId,
  isLoading = false,
  isDeleting = false,
  deleteError,
  className,
  style,
  onSelectAnnotation,
  onDeleteAnnotation,
}: ChartAnnotationListProps) {
  return (
    <section
      aria-label="チャートのアノテーション一覧"
      className={cn(
        'flex min-h-0 w-72 shrink-0 flex-col border border-border bg-card',
        className,
      )}
      style={style}
    >
      <div className="flex items-baseline justify-between border-b border-border px-3.5 py-2">
        <h2 className="font-mono text-xs font-bold uppercase tracking-wider text-foreground">
          アノテーション
        </h2>
        <span className="font-mono text-2xs text-muted-foreground">
          {isLoading ? '…' : annotations.length}
        </span>
      </div>
      {deleteError != null && (
        <p
          role="alert"
          className="border-b border-border px-3.5 py-2 text-xs text-destructive"
        >
          {deleteError}
        </p>
      )}
      {isLoading ? (
        <div className="px-3.5 py-3 font-mono text-xs text-muted-foreground">
          読み込み中
        </div>
      ) : annotations.length === 0 ? (
        <div className="px-3.5 py-3 font-mono text-xs text-muted-foreground">
          —
        </div>
      ) : (
        <div className="min-h-0 overflow-y-auto">
          {annotations.map((annotation) => {
            const isSelected = annotation.id === selectedAnnotationId

            return (
              <div
                key={annotation.id}
                className={cn(
                  'flex items-stretch border-b border-border last:border-b-0',
                  isSelected &&
                    'bg-surface-strong ring-1 ring-inset ring-primary/50',
                )}
              >
                <button
                  type="button"
                  aria-pressed={isSelected}
                  className="flex min-w-0 flex-1 flex-col gap-1 px-3.5 py-2.5 text-left hover:bg-surface-strong"
                  onClick={() => {
                    onSelectAnnotation(annotation.id)
                  }}
                >
                  <span className="flex min-w-0 items-center gap-2 font-mono text-2xs">
                    <span className="max-w-36 truncate border border-border bg-background px-1 text-muted-foreground-strong">
                      {annotation.target_kind}
                    </span>
                    <span className="text-muted-foreground">
                      {annotation.timestamp_start == null
                        ? dateFormatter.format(new Date(annotation.timestamp))
                        : `${dateFormatter.format(new Date(annotation.timestamp_start))} ～ ${dateFormatter.format(new Date(annotation.timestamp))}`}
                    </span>
                    {annotation.price != null && (
                      <span className="ml-auto text-muted-foreground">
                        {annotation.price.toLocaleString('ja-JP')}
                      </span>
                    )}
                  </span>
                  <span className="flex items-center justify-between gap-2">
                    <span className="line-clamp-2 text-sm text-foreground">
                      {buildSnippet(annotation.text)}
                    </span>
                    {annotation.created_by_kind === 'human' && (
                      <span className="shrink-0 font-mono text-2xs text-muted-foreground">
                        自分
                      </span>
                    )}
                  </span>
                </button>
                {isSelected &&
                  annotation.created_by_kind === 'human' &&
                  onDeleteAnnotation != null && (
                    <button
                      type="button"
                      aria-label={`${annotation.target_kind} のアノテーションを削除`}
                      className="shrink-0 self-center px-3 font-mono text-xs text-destructive hover:underline disabled:opacity-50"
                      disabled={isDeleting}
                      onClick={() => {
                        onDeleteAnnotation(annotation.id)
                      }}
                    >
                      削除
                    </button>
                  )}
              </div>
            )
          })}
        </div>
      )}
    </section>
  )
}
