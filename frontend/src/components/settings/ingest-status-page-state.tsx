import { Skeleton } from '#components/ui/skeleton'

export type IngestStatusPageStateVariant = 'loading' | 'error'

export function IngestStatusPageState({
  state,
}: {
  state: IngestStatusPageStateVariant
}) {
  if (state === 'loading') {
    return (
      <div aria-label="取り込み状況を読み込み中" className="space-y-6">
        <Skeleton className="h-50 w-full" />
        <div className="grid gap-6 lg:grid-cols-2">
          <Skeleton className="h-30 w-full" />
          <Skeleton className="h-30 w-full" />
        </div>
      </div>
    )
  }

  return (
    <p
      role="alert"
      className="border border-primary/40 bg-card px-4 py-5 font-mono text-xs text-primary"
    >
      取り込み状況の取得に失敗しました。時間をおいて再度ご確認ください。
    </p>
  )
}
