import { Link } from '@tanstack/react-router'
import { ChevronLeft, ChevronRight } from 'lucide-react'

import {
  type CalendarCountryFilter,
  type CalendarWeekRange,
  formatCalendarWeekRange,
  isCalendarCountryFilter,
} from '#components/calendar/calendar-date'
import { buildCalendarDays } from '#components/calendar/calendar-model'
import { Skeleton } from '#components/ui/skeleton'
import type { components } from '#lib/api/schema.gen'

type CalendarEventsResponse = components['schemas']['CalendarEventsResponse']
type Strategy = components['schemas']['Strategy']

export function CalendarPageView({
  calendar,
  weekRange,
  strategies,
  selectedStrategyId,
  countryFilter,
  isPending,
  errorMessage,
  onPreviousWeek,
  onNextWeek,
  onStrategyChange,
  onCountryChange,
}: {
  calendar: CalendarEventsResponse | undefined
  weekRange: CalendarWeekRange
  strategies: Strategy[]
  selectedStrategyId: string | undefined
  countryFilter: CalendarCountryFilter
  isPending: boolean
  errorMessage: string | undefined
  onPreviousWeek: () => void
  onNextWeek: () => void
  onStrategyChange: (value: string | undefined) => void
  onCountryChange: (value: CalendarCountryFilter) => void
}) {
  const days = buildCalendarDays(
    calendar?.events ?? [],
    countryFilter,
    selectedStrategyId,
  )

  return (
    <section className="overflow-hidden border border-border bg-background font-mono text-xs text-foreground">
      <header className="flex flex-col gap-3 border-b border-border px-3 py-3 sm:flex-row sm:items-center sm:justify-between sm:px-4">
        <div className="flex flex-wrap items-baseline gap-x-3 gap-y-1">
          <h1 className="font-semibold tracking-wide">WEEKLY EVENTS</h1>
          <span className="text-muted-foreground-strong" aria-live="polite">
            {formatCalendarWeekRange(weekRange)}
          </span>
        </div>

        <div className="flex flex-wrap items-center gap-2">
          <div className="flex items-center gap-1" aria-label="週の移動">
            <button
              type="button"
              aria-label="前の週"
              onClick={() => {
                onPreviousWeek()
              }}
              className="inline-flex size-8 items-center justify-center border border-border bg-card text-muted-foreground-strong hover:border-muted-foreground hover:text-foreground"
            >
              <ChevronLeft aria-hidden="true" className="size-4" />
            </button>
            <button
              type="button"
              aria-label="次の週"
              onClick={() => {
                onNextWeek()
              }}
              className="inline-flex size-8 items-center justify-center border border-border bg-card text-muted-foreground-strong hover:border-muted-foreground hover:text-foreground"
            >
              <ChevronRight aria-hidden="true" className="size-4" />
            </button>
          </div>

          <select
            aria-label="戦略"
            value={selectedStrategyId ?? ''}
            onChange={(event) => {
              onStrategyChange(event.target.value || undefined)
            }}
            className="h-8 max-w-44 border border-border bg-card px-2 text-xs text-foreground"
          >
            <option value="">全戦略</option>
            {strategies.map((strategy) => (
              <option key={strategy.id} value={strategy.id}>
                {strategy.name}
              </option>
            ))}
          </select>

          <select
            aria-label="国"
            value={countryFilter}
            onChange={(event) => {
              const value = event.target.value
              if (isCalendarCountryFilter(value)) onCountryChange(value)
            }}
            className="h-8 border border-border bg-card px-2 text-xs text-foreground"
          >
            <option value="all">すべての国</option>
            <option value="JP">JP</option>
            <option value="US">US</option>
            <option value="EU">EU</option>
          </select>
        </div>
      </header>

      <div aria-live="polite">
        {isPending ? (
          <div
            className="space-y-2 p-3 sm:p-4"
            aria-label="イベントを読み込み中"
          >
            <Skeleton className="h-6 w-full rounded-none" />
            <Skeleton className="h-18 w-full rounded-none" />
            <Skeleton className="h-18 w-full rounded-none" />
          </div>
        ) : errorMessage != null ? (
          <p role="alert" className="px-4 py-8 text-center text-destructive">
            {errorMessage}
          </p>
        ) : days.length === 0 ? (
          <p className="px-4 py-8 text-center text-muted-foreground-strong">
            この週に予定はありません。
          </p>
        ) : (
          <div>
            {days.map((day) => (
              <section key={day.date} aria-label={day.label}>
                <h2 className="border-t border-border px-3 py-1.5 text-muted-foreground-strong first:border-t-0 sm:px-4">
                  {day.label}
                </h2>
                {day.rows.map((row) => (
                  <div
                    key={row.key}
                    className="flex items-start gap-1.5 px-3 py-1 sm:gap-2 sm:px-4"
                  >
                    <span className="w-18 shrink-0 pt-0.5 text-right tabular-nums text-muted-foreground">
                      {row.time}
                    </span>
                    <span className="w-10 shrink-0 border border-border px-1 py-0.5 text-center text-2xs text-muted-foreground-strong">
                      {row.country}
                    </span>
                    <span className="w-12 shrink-0 border border-border px-1 py-0.5 text-center text-2xs text-muted-foreground-strong">
                      {row.category}
                    </span>
                    {row.stockId != null ? (
                      <Link
                        to="/charts/$instrumentId"
                        params={{ instrumentId: row.stockId }}
                        className={`min-w-0 flex-1 break-words hover:underline ${
                          row.emphasized
                            ? 'text-destructive'
                            : row.target
                              ? 'font-semibold text-foreground'
                              : 'text-foreground'
                        }`}
                      >
                        {row.title}
                      </Link>
                    ) : (
                      <span
                        className={`min-w-0 flex-1 break-words ${
                          row.emphasized
                            ? 'text-destructive'
                            : row.target
                              ? 'font-semibold text-foreground'
                              : row.category === '決算' &&
                                  row.title.startsWith('他 ')
                                ? 'text-muted-foreground'
                                : 'text-foreground'
                        }`}
                      >
                        {row.title}
                      </span>
                    )}
                  </div>
                ))}
              </section>
            ))}
          </div>
        )}
      </div>
    </section>
  )
}
