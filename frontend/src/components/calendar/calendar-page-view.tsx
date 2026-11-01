import { Button } from '@fohte/ui/button'
import { ButtonGroup } from '@fohte/ui/button-group'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@fohte/ui/select'
import { ChevronLeft, ChevronRight } from 'lucide-react'

import {
  type CalendarCountryFilter,
  type CalendarWeekRange,
  formatCalendarWeekRange,
  isCalendarCountryFilter,
} from '#components/calendar/calendar-date'
import { CalendarEventRow } from '#components/calendar/calendar-event-row'
import {
  buildCalendarDays,
  isSameOtherEarningsSelection,
  type OtherEarningsSelection,
} from '#components/calendar/calendar-model'
import { Skeleton } from '#components/ui/skeleton'
import type { components } from '#lib/api/schema.gen'

type CalendarEventsResponse = components['schemas']['CalendarEventsResponse']
type CalendarEvent = components['schemas']['CalendarEventResponse']
type Strategy = components['schemas']['Strategy']

const COUNTRY_FILTER_OPTIONS = [
  { value: null, label: 'すべての国' },
  { value: 'JP', label: 'JP' },
  { value: 'US', label: 'US' },
  { value: 'EU', label: 'EU' },
]

export function CalendarPageView({
  calendar,
  today,
  weekRange,
  strategies,
  selectedStrategyId,
  countryFilter,
  isPending,
  errorMessage,
  strategyErrorMessage,
  expandedOtherEarnings,
  otherEarningsEvents,
  isOtherEarningsPending,
  otherEarningsErrorMessage,
  onPreviousWeek,
  onNextWeek,
  onStrategyChange,
  onCountryChange,
  onOtherEarningsToggle,
  openFilter,
}: {
  calendar: CalendarEventsResponse | undefined
  today: string
  weekRange: CalendarWeekRange
  strategies: Strategy[]
  selectedStrategyId: string | undefined
  countryFilter: CalendarCountryFilter
  isPending: boolean
  errorMessage: string | undefined
  strategyErrorMessage: string | undefined
  expandedOtherEarnings: OtherEarningsSelection | undefined
  otherEarningsEvents: CalendarEvent[] | undefined
  isOtherEarningsPending: boolean
  otherEarningsErrorMessage: string | undefined
  onPreviousWeek: () => void
  onNextWeek: () => void
  onStrategyChange: (value: string | undefined) => void
  onCountryChange: (value: CalendarCountryFilter) => void
  onOtherEarningsToggle: (selection: OtherEarningsSelection) => void
  openFilter?: 'country' | 'strategy'
}) {
  const days = buildCalendarDays(
    calendar?.events ?? [],
    countryFilter,
    selectedStrategyId,
    today,
  )
  const otherEarningsRows = buildCalendarDays(
    otherEarningsEvents ?? [],
    countryFilter,
    undefined,
    today,
  ).flatMap((day) => day.rows)
  const strategyOptions = [
    { value: null, label: '全戦略' },
    ...strategies.map((strategy) => ({
      value: strategy.id,
      label: strategy.name,
    })),
  ]

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
          <ButtonGroup aria-label="週の移動" className="shrink-0">
            <Button
              type="button"
              variant="plain"
              aria-label="前の週"
              onClick={() => {
                onPreviousWeek()
              }}
              className="size-8"
            >
              <ChevronLeft aria-hidden="true" className="size-4" />
            </Button>
            <Button
              type="button"
              variant="plain"
              aria-label="次の週"
              onClick={() => {
                onNextWeek()
              }}
              className="size-8"
            >
              <ChevronRight aria-hidden="true" className="size-4" />
            </Button>
          </ButtonGroup>

          <div className="grid gap-1">
            <Select
              items={strategyOptions}
              value={selectedStrategyId ?? null}
              open={openFilter === 'strategy' ? true : undefined}
              onValueChange={(value) => {
                onStrategyChange(value ?? undefined)
              }}
            >
              <SelectTrigger
                aria-label="戦略"
                aria-invalid={strategyErrorMessage != null}
                aria-describedby={
                  strategyErrorMessage != null
                    ? 'calendar-strategy-error'
                    : undefined
                }
                className="max-w-44"
              >
                <SelectValue placeholder="全戦略" />
              </SelectTrigger>
              <SelectContent>
                {strategyOptions.map(({ value, label }) => (
                  <SelectItem key={value ?? 'all-strategies'} value={value}>
                    {label}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
            {strategyErrorMessage != null && (
              <p
                id="calendar-strategy-error"
                role="alert"
                className="max-w-44 text-2xs text-destructive"
              >
                {strategyErrorMessage}
              </p>
            )}
          </div>

          <Select
            items={COUNTRY_FILTER_OPTIONS}
            value={countryFilter === 'all' ? null : countryFilter}
            open={openFilter === 'country' ? true : undefined}
            onValueChange={(value) => {
              const next = value ?? 'all'
              if (isCalendarCountryFilter(next)) onCountryChange(next)
            }}
          >
            <SelectTrigger aria-label="国">
              <SelectValue placeholder="すべての国" />
            </SelectTrigger>
            <SelectContent alignItemWithTrigger={false}>
              {COUNTRY_FILTER_OPTIONS.map(({ value, label }) => (
                <SelectItem key={value ?? 'all-countries'} value={value}>
                  {label}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </div>
      </header>

      <div aria-live="polite">
        {isPending ? (
          <div
            className="space-y-2 p-3 sm:p-4"
            aria-label="イベントを読み込み中"
          >
            <Skeleton className="h-6 w-full" />
            <Skeleton className="h-18 w-full" />
            <Skeleton className="h-18 w-full" />
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
                {day.rows.map((row) => {
                  const otherEarnings = row.otherEarnings
                  const isExpanded =
                    otherEarnings != null &&
                    isSameOtherEarningsSelection(
                      expandedOtherEarnings,
                      otherEarnings,
                    )
                  const detailsId =
                    otherEarnings == null
                      ? undefined
                      : `calendar-other-earnings-${otherEarnings.country}-${otherEarnings.eventDate}`

                  return (
                    <div key={row.key}>
                      <CalendarEventRow
                        time={row.time}
                        country={row.country}
                        category={row.category}
                        title={row.title}
                        stockId={row.stockId}
                        earningsStatus={row.earningsStatus}
                        indicatorStatus={row.indicatorStatus}
                        centralBankStatus={row.centralBankStatus}
                        emphasized={row.emphasized}
                        target={row.target}
                        muted={row.muted}
                        titleAction={
                          otherEarnings == null
                            ? undefined
                            : {
                                expanded: isExpanded,
                                controlsId: isExpanded ? detailsId : undefined,
                                onClick: () => {
                                  onOtherEarningsToggle(otherEarnings)
                                },
                              }
                        }
                      />
                      {isExpanded && (
                        <div id={detailsId} aria-live="polite">
                          {isOtherEarningsPending ? (
                            <div
                              className="space-y-1 px-3 py-2 sm:px-4"
                              aria-label="他の決算を読み込み中"
                            >
                              <Skeleton className="h-5 w-full" />
                              <Skeleton className="h-5 w-full" />
                            </div>
                          ) : otherEarningsErrorMessage != null ? (
                            <p
                              role="alert"
                              className="px-4 py-2 text-destructive"
                            >
                              {otherEarningsErrorMessage}
                            </p>
                          ) : otherEarningsRows.length === 0 ? (
                            <p className="px-4 py-2 text-muted-foreground-strong">
                              該当する決算はありません。
                            </p>
                          ) : (
                            otherEarningsRows.map((earning) => (
                              <CalendarEventRow
                                key={earning.key}
                                time={earning.time}
                                country={earning.country}
                                category={earning.category}
                                title={earning.title}
                                stockId={earning.stockId}
                                earningsStatus={earning.earningsStatus}
                                emphasized={earning.emphasized}
                                target={earning.target}
                                muted={earning.muted}
                              />
                            ))
                          )}
                        </div>
                      )}
                    </div>
                  )
                })}
              </section>
            ))}
          </div>
        )}
      </div>
    </section>
  )
}
