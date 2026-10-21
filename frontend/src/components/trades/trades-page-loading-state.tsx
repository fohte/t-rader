import { Skeleton } from '#components/ui/skeleton'

export function TradesPageLoadingState({
  kind,
}: {
  kind: 'summary' | 'table'
}) {
  return kind === 'summary' ? (
    <Skeleton className="h-22 w-full" />
  ) : (
    <Skeleton className="h-50 w-full" />
  )
}
