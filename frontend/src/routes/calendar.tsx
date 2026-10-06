import { createFileRoute } from '@tanstack/react-router'
import { useMemo } from 'react'

import {
  getCalendarWeekRange,
  getTokyoDate,
  shiftCalendarWeek,
  validateCalendarSearch,
} from '#components/calendar/calendar-date'
import { CalendarPageView } from '#components/calendar/calendar-page-view'
import { $api } from '#lib/api/client'

export const Route = createFileRoute('/calendar')({
  validateSearch: validateCalendarSearch,
  component: CalendarRoute,
})

function CalendarRoute() {
  const { country, strategy, week } = Route.useSearch()
  const navigate = Route.useNavigate()
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
  const { data: strategies = [] } = $api.useQuery('get', '/api/strategies')

  return (
    <CalendarPageView
      calendar={calendar}
      weekRange={weekRange}
      strategies={strategies}
      selectedStrategyId={strategy}
      countryFilter={country ?? 'all'}
      isPending={isPending}
      errorMessage={isError ? 'イベントの取得に失敗しました' : undefined}
      onPreviousWeek={() => {
        void navigate({
          search: (previous) => ({
            ...previous,
            week: shiftCalendarWeek(weekRange.from, -1),
          }),
        })
      }}
      onNextWeek={() => {
        void navigate({
          search: (previous) => ({
            ...previous,
            week: shiftCalendarWeek(weekRange.from, 1),
          }),
        })
      }}
      onStrategyChange={(value) => {
        void navigate({
          search: (previous) => ({ ...previous, strategy: value }),
        })
      }}
      onCountryChange={(value) => {
        void navigate({
          search: (previous) => ({
            ...previous,
            country: value === 'all' ? undefined : value,
          }),
        })
      }}
    />
  )
}
