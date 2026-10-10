import type { ChartAnnotation } from '#lib/annotation-chart-utils'
import { buildSnippet } from '#lib/note-utils'
import { cn } from '#lib/utils'

interface ChartAnnotationListProps {
  annotations: ChartAnnotation[]
  selectedAnnotationId: string | null
  isLoading?: boolean
  className?: string
  onSelectAnnotation: (id: string) => void
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
  className,
  onSelectAnnotation,
}: ChartAnnotationListProps) {
  return (
    <section
      aria-label="チャートのアノテーション一覧"
      className={cn(
        'flex min-h-0 w-72 shrink-0 flex-col border border-border bg-card',
        className,
      )}
    >
      <div className="flex items-baseline justify-between border-b border-border px-3.5 py-2">
        <h2 className="font-mono text-xs font-bold uppercase tracking-wider text-foreground">
          アノテーション
        </h2>
        <span className="font-mono text-2xs text-muted-foreground">
          {isLoading ? '…' : annotations.length}
        </span>
      </div>
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
              <button
                key={annotation.id}
                type="button"
                aria-pressed={isSelected}
                className={cn(
                  'flex w-full flex-col gap-1 border-b border-border px-3.5 py-2.5 text-left last:border-b-0 hover:bg-surface-strong',
                  isSelected &&
                    'bg-surface-strong ring-1 ring-inset ring-primary/50',
                )}
                onClick={() => {
                  onSelectAnnotation(annotation.id)
                }}
              >
                <span className="flex min-w-0 items-center gap-2 font-mono text-2xs">
                  <span className="max-w-36 truncate border border-border bg-background px-1 text-muted-foreground-strong">
                    {annotation.target_kind}
                  </span>
                  <span className="text-muted-foreground">
                    {dateFormatter.format(new Date(annotation.timestamp))}
                  </span>
                  {annotation.price != null && (
                    <span className="ml-auto text-muted-foreground">
                      {annotation.price.toLocaleString('ja-JP')}
                    </span>
                  )}
                </span>
                <span className="line-clamp-2 text-sm text-foreground">
                  {buildSnippet(annotation.text)}
                </span>
              </button>
            )
          })}
        </div>
      )}
    </section>
  )
}
