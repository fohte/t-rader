import type { ReviewQueueData } from '#components/dashboard/dashboard-model'
import { DashboardRecentNotes } from '#components/dashboard/dashboard-recent-notes'
import { DashboardRecentRuns } from '#components/dashboard/dashboard-recent-runs'
import { DashboardRecentTrades } from '#components/dashboard/dashboard-recent-trades'
import { DashboardReviewQueue } from '#components/dashboard/dashboard-review-queue'
import { DashboardStats } from '#components/dashboard/dashboard-stats'
import { DashboardWeeklyEvents } from '#components/dashboard/dashboard-weekly-events'
import { PositionsTable } from '#components/portfolio/positions-table'
import type { StatItem } from '#components/portfolio/stat-row'
import { Skeleton } from '#components/ui/skeleton'
import type { components } from '#lib/api/schema.gen'

type CalendarEvent = components['schemas']['CalendarEventResponse']
type Note = components['schemas']['Note']
type PositionSummary = components['schemas']['PositionSummary']
type Stock = components['schemas']['Stock']
type Strategy = components['schemas']['Strategy']
type Task = components['schemas']['StrategyTaskSummary']
type Trade = components['schemas']['TradeListItem']

interface DashboardPageViewProps {
  stats: StatItem[] | null
  isStatsError: boolean
  positions: PositionSummary[] | null
  isPositionsError: boolean
  stocks: Stock[]
  reviewQueue: ReviewQueueData | null
  isReviewPending: boolean
  isReviewError: boolean
  trades: Trade[] | null
  onReflectTrade: (trade: Trade) => void
  events: CalendarEvent[] | null
  notes: Note[] | null
  tasks: Task[] | null
  strategies: Strategy[]
}

export function DashboardPageView({
  stats,
  isStatsError,
  positions,
  isPositionsError,
  stocks,
  reviewQueue,
  isReviewPending,
  isReviewError,
  trades,
  onReflectTrade,
  events,
  notes,
  tasks,
  strategies,
}: DashboardPageViewProps) {
  return (
    <div className="space-y-5 font-sans text-foreground">
      <header>
        <h1 className="mb-1.5 text-2xl font-bold leading-tight tracking-tight">
          ダッシュボード
        </h1>
        <p className="max-w-180 text-sm leading-relaxed text-muted-foreground-strong">
          資産と直近の売買、最近のノート、レビュー待ち。
        </p>
      </header>

      <div className="grid grid-cols-1 gap-5 lg:grid-cols-(--grid-cols-portfolio-layout)">
        <div className="min-w-0 space-y-5">
          <DashboardStats stats={stats} isError={isStatsError} />
          <div className="lg:hidden">
            <DashboardReviewQueue
              data={reviewQueue}
              isPending={isReviewPending}
              isError={isReviewError}
              variant="compact"
            />
          </div>

          <section
            className="space-y-2"
            aria-labelledby="dashboard-positions-title"
          >
            <div className="flex items-baseline justify-between gap-3">
              <h2
                id="dashboard-positions-title"
                className="font-mono text-2xs uppercase tracking-wider text-muted-foreground"
              >
                <span className="mr-2 text-primary">&gt;</span>
                保有銘柄
              </h2>
              <span className="shrink-0 font-mono text-2xs text-muted-foreground">
                {positions == null ? '…' : `${String(positions.length)} 銘柄`}
              </span>
            </div>
            {isPositionsError ? (
              <p className="border border-border bg-card px-4 py-5 font-mono text-xs text-muted-foreground">
                保有銘柄を読み込めませんでした。
              </p>
            ) : positions == null ? (
              <Skeleton className="h-40 w-full" />
            ) : (
              <PositionsTable positions={positions} stocks={stocks} />
            )}
          </section>

          <DashboardRecentTrades
            trades={trades}
            stocks={stocks}
            onReflect={onReflectTrade}
          />
          <DashboardWeeklyEvents events={events} stocks={stocks} />
        </div>

        <aside className="min-w-0 space-y-5">
          <div className="hidden lg:block">
            <DashboardReviewQueue
              data={reviewQueue}
              isPending={isReviewPending}
              isError={isReviewError}
              variant="panel"
            />
          </div>
          <DashboardRecentNotes notes={notes} />
          <DashboardRecentRuns tasks={tasks} strategies={strategies} />
        </aside>
      </div>
    </div>
  )
}
