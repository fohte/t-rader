import { AnnotationDraftForm } from '#components/stock-page/annotation-draft-form'
import {
  orderTrendLinePoints,
  type TrendLineAnnotationDraft,
  type TrendLinePoint,
} from '#lib/annotation-chart-utils'

const dateFormatter = new Intl.DateTimeFormat('ja-JP', {
  month: '2-digit',
  day: '2-digit',
  timeZone: 'Asia/Tokyo',
})

const priceFormatter = new Intl.NumberFormat('ja-JP')

export function TrendLineEditor({
  start,
  end,
  targetKinds,
  isSaving = false,
  errorMessage,
  onSave,
  onCancel,
}: {
  start: TrendLinePoint
  end: TrendLinePoint
  targetKinds: string[]
  isSaving?: boolean
  errorMessage?: string | null
  onSave: (draft: TrendLineAnnotationDraft) => void
  onCancel: () => void
}) {
  const [orderedStart, orderedEnd] = orderTrendLinePoints(start, end)

  return (
    <AnnotationDraftForm
      ariaLabel="トレンドラインのアノテーション"
      header={
        <div className="grid gap-1">
          <h2 className="font-mono text-xs font-bold text-foreground">
            新しい線
          </h2>
          <span className="font-mono text-2xs text-muted-foreground">
            トレンドライン{' '}
            {dateFormatter.format(new Date(orderedStart.timestamp))}{' '}
            {priceFormatter.format(orderedStart.price)} →{' '}
            {dateFormatter.format(new Date(orderedEnd.timestamp))}{' '}
            {priceFormatter.format(orderedEnd.price)}
          </span>
        </div>
      }
      targetKinds={targetKinds}
      isSaving={isSaving}
      errorMessage={errorMessage}
      onSave={({ target_kind, text }) => {
        onSave({ start, end, target_kind, text })
      }}
      onCancel={onCancel}
    />
  )
}
