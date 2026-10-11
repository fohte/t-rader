import { type StatItem, StatRow } from '#components/portfolio/stat-row'
import { Skeleton } from '#components/ui/skeleton'

export function DashboardStats({
  stats,
  isError,
}: {
  stats: StatItem[] | null
  isError: boolean
}) {
  return isError ? (
    <p className="border border-border bg-card px-4 py-5 font-mono text-xs text-muted-foreground">
      資産情報を読み込めませんでした。
    </p>
  ) : stats == null ? (
    <Skeleton className="h-22 w-full" />
  ) : (
    <StatRow stats={stats} />
  )
}
