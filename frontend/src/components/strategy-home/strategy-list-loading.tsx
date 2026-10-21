import { Skeleton } from '#components/ui/skeleton'

export function StrategyListLoading() {
  return (
    <div className="grid grid-cols-1 gap-3.5 sm:grid-cols-2 lg:grid-cols-3">
      <Skeleton className="h-47" />
      <Skeleton className="h-47" />
      <Skeleton className="h-47" />
    </div>
  )
}
