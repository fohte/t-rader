import { AnnotationDraftForm } from '#components/stock-page/annotation-draft-form'
import type { HorizontalLineAnnotationDraft } from '#lib/annotation-chart-utils'

export function HorizontalLineEditor({
  timestamp,
  price,
  targetKinds,
  isSaving = false,
  errorMessage,
  onSave,
  onCancel,
}: {
  timestamp: string
  price: number
  targetKinds: string[]
  isSaving?: boolean
  errorMessage?: string | null
  onSave: (draft: HorizontalLineAnnotationDraft) => void
  onCancel: () => void
}) {
  return (
    <AnnotationDraftForm
      ariaLabel="水平線のアノテーション"
      header={
        <div className="flex items-baseline justify-between gap-2">
          <h2 className="font-mono text-xs font-bold text-foreground">
            新しい水平線
          </h2>
          <span className="font-mono text-2xs text-muted-foreground">
            {new Date(timestamp).toLocaleDateString('ja-JP')} ·{' '}
            {price.toLocaleString('ja-JP')}
          </span>
        </div>
      }
      targetKinds={targetKinds}
      isSaving={isSaving}
      errorMessage={errorMessage}
      onSave={({ target_kind, text }) => {
        onSave({ target_kind, text, timestamp, price })
      }}
      onCancel={onCancel}
    />
  )
}
