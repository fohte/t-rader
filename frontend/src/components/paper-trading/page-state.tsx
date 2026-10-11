import { Skeleton } from '#components/ui/skeleton'

type PaperTradingPageState = 'loading' | 'error' | 'not-found'

export function PaperTradingPageStateView({
  state,
}: {
  state: PaperTradingPageState
}) {
  if (state === 'loading') {
    return (
      <div aria-busy="true" className="space-y-4">
        <Skeleton className="h-8 w-72" />
        <Skeleton className="h-5 w-full max-w-120" />
        <Skeleton className="h-50 w-full" />
      </div>
    )
  }

  return (
    <div
      role={state === 'error' ? 'alert' : 'status'}
      className="border border-border bg-card px-4 py-8 text-center font-mono text-xs text-muted-foreground"
    >
      {state === 'error'
        ? 'ペーパートレード情報を取得できませんでした。'
        : '口座が見つかりませんでした。'}
    </div>
  )
}
