import { Link } from '@tanstack/react-router'

import { formatCalendarDayLabel } from '#components/calendar/calendar-date'
import { Skeleton } from '#components/ui/skeleton'
import type { components } from '#lib/api/schema.gen'

type CalendarEvent = components['schemas']['CalendarEventResponse']
type Stock = components['schemas']['Stock']

export function DashboardWeeklyEvents({
  events,
  stocks,
}: {
  events: CalendarEvent[] | null
  stocks: Stock[]
}) {
  const upcomingEvents =
    events == null
      ? []
      : [...events]
          .sort((left, right) => {
            const dateOrder = left.event_date.localeCompare(right.event_date)
            if (dateOrder !== 0) return dateOrder
            if (left.kind === 'event' && right.kind === 'event') {
              return (left.event_at ?? '').localeCompare(right.event_at ?? '')
            }
            return 0
          })
          .slice(0, 4)
  const stockById = new Map(stocks.map((stock) => [stock.id, stock.name]))

  return (
    <section className="space-y-2" aria-labelledby="dashboard-events-title">
      <div className="flex items-baseline justify-between gap-3">
        <h2
          id="dashboard-events-title"
          className="font-mono text-2xs uppercase tracking-wider text-muted-foreground"
        >
          <span className="mr-2 text-primary">&gt;</span>
          今週の予定
        </h2>
        <Link
          to="/calendar"
          className="shrink-0 font-mono text-2xs text-primary hover:underline"
        >
          カレンダー →
        </Link>
      </div>
      <div className="divide-y divide-border border border-border bg-card">
        {events == null ? (
          <div className="space-y-2 p-3">
            <Skeleton className="h-8 w-full" />
            <Skeleton className="h-8 w-full" />
          </div>
        ) : upcomingEvents.length === 0 ? (
          <p className="px-4 py-7 text-center font-mono text-xs text-muted-foreground">
            今週の予定はありません。
          </p>
        ) : (
          upcomingEvents.map((event) => {
            const title =
              event.kind === 'event'
                ? event.title
                : `その他 ${String(event.count)} 社の決算`
            const category = event.kind === 'event' ? event.category : '決算'
            const stockName =
              event.kind === 'event' && event.stock_id != null
                ? stockById.get(event.stock_id)
                : undefined

            return (
              <div
                key={`${event.event_date}:${event.country}:${title}`}
                className="flex min-w-0 items-start gap-2 px-3 py-2.5 sm:px-3.5"
              >
                <span className="w-20 shrink-0 pt-0.5 font-mono text-2xs tabular-nums text-muted-foreground">
                  {formatCalendarDayLabel(event.event_date)}
                </span>
                <span className="shrink-0 border border-border px-1 py-0.5 font-mono text-2xs text-muted-foreground-strong">
                  {category}
                </span>
                {event.kind === 'event' && event.stock_id != null ? (
                  <Link
                    to="/charts/$instrumentId"
                    params={{ instrumentId: event.stock_id }}
                    className="min-w-0 flex-1 break-words text-sm text-foreground hover:underline"
                  >
                    {stockName ?? title}
                    {stockName != null && (
                      <span className="ml-1.5 font-mono text-2xs text-muted-foreground">
                        {event.stock_id}
                      </span>
                    )}
                  </Link>
                ) : (
                  <span className="min-w-0 flex-1 break-words text-sm text-foreground">
                    {title}
                  </span>
                )}
              </div>
            )
          })
        )}
      </div>
    </section>
  )
}
