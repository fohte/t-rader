import { createFileRoute } from '@tanstack/react-router'
import { useMemo, useState } from 'react'

import {
  getCalendarWeekRange,
  getTokyoDate,
  shiftCalendarWeek,
  validateCalendarSearch,
} from '#components/calendar/calendar-date'
import {
  isSameOtherEarningsSelection,
  type OtherEarningsSelection,
} from '#components/calendar/calendar-model'
import { CalendarPageView } from '#components/calendar/calendar-page-view'
import { $api } from '#lib/api/client'

export const Route = createFileRoute('/calendar')({
  validateSearch: validateCalendarSearch,
  component: CalendarRoute,
})

function CalendarRoute() {
  const { country, strategy, week } = Route.useSearch()
  const navigate = Route.useNavigate()
  const [expandedOtherEarnings, setExpandedOtherEarnings] =
    useState<OtherEarningsSelection>()
  const weekRange = useMemo(
    () => getCalendarWeekRange(week ?? getTokyoDate()),
    [week],
  )
  const {
    data: calendar,
    isPending,
    isError,
  } = $api.useQuery('get', '/api/calendar/events', {
    params: {
      query: {
        from: weekRange.from,
        to: weekRange.to,
        strategy_id: strategy,
      },
    },
  })
  const isExpandedSummaryVisible =
    expandedOtherEarnings != null &&
    (country == null || country === expandedOtherEarnings.country) &&
    calendar?.events.some(
      (event) =>
        event.kind === 'other_earnings_summary' &&
        event.country === expandedOtherEarnings.country &&
        event.event_date === expandedOtherEarnings.eventDate,
    ) === true
  const activeExpandedOtherEarnings = isExpandedSummaryVisible
    ? expandedOtherEarnings
    : undefined
  const otherEarningsQuery = $api.useQuery(
    'get',
    '/api/calendar/other-earnings',
    {
      params: {
        query: {
          country: activeExpandedOtherEarnings?.country ?? 'JP',
          event_date: activeExpandedOtherEarnings?.eventDate ?? weekRange.from,
          strategy_id: strategy,
        },
      },
    },
    { enabled: activeExpandedOtherEarnings != null },
  )
  const { data: strategies = [], isError: isStrategyError } = $api.useQuery(
    'get',
    '/api/strategies',
  )

  return (
    <CalendarPageView
      calendar={calendar}
      weekRange={weekRange}
      strategies={strategies}
      selectedStrategyId={strategy}
      countryFilter={country ?? 'all'}
      isPending={isPending}
      errorMessage={isError ? 'イベントの取得に失敗しました' : undefined}
      strategyErrorMessage={
        isStrategyError ? '戦略一覧の取得に失敗しました' : undefined
      }
      expandedOtherEarnings={activeExpandedOtherEarnings}
      otherEarningsEvents={otherEarningsQuery.data?.events}
      isOtherEarningsPending={otherEarningsQuery.isPending}
      otherEarningsErrorMessage={
        otherEarningsQuery.isError ? '決算一覧の取得に失敗しました' : undefined
      }
      onPreviousWeek={() => {
        setExpandedOtherEarnings(undefined)
        void navigate({
          search: (previous) => ({
            ...previous,
            week: shiftCalendarWeek(weekRange.from, -1),
          }),
        })
      }}
      onNextWeek={() => {
        setExpandedOtherEarnings(undefined)
        void navigate({
          search: (previous) => ({
            ...previous,
            week: shiftCalendarWeek(weekRange.from, 1),
          }),
        })
      }}
      onStrategyChange={(value) => {
        setExpandedOtherEarnings(undefined)
        void navigate({
          search: (previous) => ({ ...previous, strategy: value }),
        })
      }}
      onCountryChange={(value) => {
        setExpandedOtherEarnings(undefined)
        void navigate({
          search: (previous) => ({
            ...previous,
            country: value === 'all' ? undefined : value,
          }),
        })
      }}
      onOtherEarningsToggle={(selection) => {
        setExpandedOtherEarnings((current) =>
          isSameOtherEarningsSelection(current, selection)
            ? undefined
            : selection,
        )
      }}
    />
  )
}
