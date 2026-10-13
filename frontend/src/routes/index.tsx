import { useQueries } from '@tanstack/react-query'
import { createFileRoute } from '@tanstack/react-router'
import { useMemo } from 'react'

import {
  getCalendarWeekRange,
  getTokyoDate,
} from '#components/calendar/calendar-date'
import { DashboardPageView } from '#components/dashboard/dashboard-page-view'
import { useReviewQueue } from '#components/dashboard/use-review-queue'
import type { StatItem } from '#components/portfolio/stat-row'
import { formatYen, pnlColorClass } from '#components/trades/format'
import { $api } from '#lib/api/client'

export const Route = createFileRoute('/')({
  component: DashboardRoute,
})

function DashboardRoute() {
  const summaryQuery = $api.useQuery('get', '/api/trades/summary')
  const stocksQuery = $api.useQuery('get', '/api/refs/stocks')
  const strategiesQuery = $api.useQuery('get', '/api/strategies')
  const tradesQuery = $api.useQuery('get', '/api/trades')
  const notesQuery = $api.useQuery('get', '/api/notes')
  const taskQuery = $api.useQuery('get', '/api/tasks')
  const reviewQueue = useReviewQueue()
  const today = getTokyoDate()
  const weekRange = useMemo(() => getCalendarWeekRange(today), [today])
  const eventsQuery = $api.useQuery('get', '/api/calendar/events', {
    params: { query: { from: weekRange.from, to: weekRange.to } },
  })
  const strategies = strategiesQuery.data ?? []
  const investableAmountQueries = useQueries({
    queries: strategies.map((strategy) =>
      $api.queryOptions('get', '/api/strategies/{id}/investable-amount', {
        params: { path: { id: strategy.id } },
      }),
    ),
  })
  const isStatsError =
    summaryQuery.isError ||
    strategiesQuery.isError ||
    investableAmountQueries.some((query) => query.isError)
  const positions = useMemo(
    () =>
      summaryQuery.data?.positions.filter((position) => position.qty > 0) ??
      null,
    [summaryQuery.data],
  )
  const stats = useMemo<StatItem[] | null>(() => {
    if (
      summaryQuery.data == null ||
      strategiesQuery.isPending ||
      investableAmountQueries.some((query) => query.isPending)
    ) {
      return null
    }

    const equity = (positions ?? []).reduce(
      (sum, position) => sum + position.cost_basis,
      0,
    )
    const totalAssets = investableAmountQueries.reduce(
      (sum, query) => sum + (query.data?.amount_jpy ?? 0),
      0,
    )
    const cash = totalAssets - equity
    const realizedPnl = summaryQuery.data.realized_pnl

    return [
      { label: '総資産', value: formatYen(totalAssets) },
      { label: '評価額 (株式・簿価)', value: formatYen(equity) },
      { label: '現金', value: formatYen(cash) },
      {
        label: '実現損益 (累計)',
        value: formatYen(realizedPnl, true),
        cls: pnlColorClass(realizedPnl),
      },
    ]
  }, [
    investableAmountQueries,
    positions,
    strategiesQuery.isPending,
    summaryQuery.data,
  ])
  const events = useMemo(
    () =>
      eventsQuery.data?.events.filter((event) => event.event_date >= today) ??
      null,
    [eventsQuery.data, today],
  )

  return (
    <>
      <DashboardPageView
        stats={stats}
        isStatsError={isStatsError}
        positions={
          summaryQuery.isPending || summaryQuery.isError
            ? null
            : (positions ?? [])
        }
        isPositionsError={summaryQuery.isError}
        stocks={stocksQuery.data ?? []}
        reviewQueue={reviewQueue.data}
        isReviewPending={reviewQueue.isPending}
        isReviewError={reviewQueue.isError}
        trades={tradesQuery.isPending ? null : (tradesQuery.data ?? [])}
        isTradesError={tradesQuery.isError}
        events={eventsQuery.isPending ? null : events}
        isEventsError={eventsQuery.isError}
        notes={notesQuery.isPending ? null : (notesQuery.data ?? [])}
        isNotesError={notesQuery.isError}
        tasks={taskQuery.isPending ? null : (taskQuery.data ?? [])}
        isTasksError={taskQuery.isError}
        strategies={strategies}
      />
    </>
  )
}
