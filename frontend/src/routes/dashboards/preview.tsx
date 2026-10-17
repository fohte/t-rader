import { createFileRoute } from '@tanstack/react-router'
import { useMemo, useState } from 'react'

import { getTokyoDate } from '#components/calendar/calendar-date'
import {
  getInitialDashboardDate,
  parseDashboardPreviewHash,
} from '#components/dashboard-pages/dashboard-config'
import { DashboardPageView } from '#components/dashboard-pages/dashboard-page-view'
import { useDashboardWidgetQueries } from '#components/dashboard-pages/use-dashboard-widget-queries'

export const Route = createFileRoute('/dashboards/preview')({
  component: DashboardPreviewRoute,
})

function DashboardPreviewRoute() {
  const parsedConfig = useMemo(
    () => parseDashboardPreviewHash(window.location.hash),
    [],
  )
  const [selectedDate, setSelectedDate] = useState<string | null>(null)
  const defaultDate = useMemo(() => getTokyoDate(), [])
  const config = parsedConfig.ok ? parsedConfig.value : null
  const asOf =
    selectedDate ??
    (config == null
      ? defaultDate
      : getInitialDashboardDate(config, defaultDate))
  const widgets = useDashboardWidgetQueries(config, asOf)

  if (!parsedConfig.ok) {
    return (
      <DashboardPageView
        state={{
          status: 'invalid',
          title: 'プレビュー',
          issues: parsedConfig.issues,
        }}
      />
    )
  }

  return (
    <DashboardPageView
      state={{
        status: 'ready',
        title: parsedConfig.value.title,
        asOf,
        onAsOfChange: (date) => {
          if (date !== '') setSelectedDate(date)
        },
        widgets,
      }}
    />
  )
}
