import { useState } from 'react'

import type { DashboardConfig } from '#components/dashboard-pages/dashboard-config'
import type { DashboardWidgetPageState } from '#components/dashboard-pages/dashboard-page-view'
import { useDashboardWidgetQueries } from '#components/dashboard-pages/use-dashboard-widget-queries'

type DashboardDateSelection = {
  resetKey: string
  date: string
}

export function useDashboardPageData(
  config: DashboardConfig | null,
  resetKey: string,
): {
  asOf: string | null
  onAsOfChange: (date: string) => void
  widgets: DashboardWidgetPageState[]
} {
  const [dateSelection, setDateSelection] =
    useState<DashboardDateSelection | null>(null)
  const selectedDate =
    dateSelection?.resetKey === resetKey ? dateSelection.date : null
  const hasConfiguredAsOf =
    config?.widgets.some(
      (widget) => typeof widget.definition.time?.as_of === 'string',
    ) ?? false
  let asOf: string | null = selectedDate ?? ''
  if (selectedDate === null && hasConfiguredAsOf) asOf = null

  let queryAsOf: string | null | undefined = selectedDate
  if (selectedDate === null) queryAsOf = hasConfiguredAsOf ? undefined : null
  if (selectedDate === '') queryAsOf = null

  const widgets = useDashboardWidgetQueries(config, queryAsOf)

  return {
    asOf,
    onAsOfChange: (date) => {
      setDateSelection({ resetKey, date })
    },
    widgets,
  }
}
