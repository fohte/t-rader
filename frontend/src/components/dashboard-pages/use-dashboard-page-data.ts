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
  asOf: string
  onAsOfChange: (date: string) => void
  widgets: DashboardWidgetPageState[]
} {
  const [dateSelection, setDateSelection] =
    useState<DashboardDateSelection | null>(null)
  const selectedDate =
    dateSelection?.resetKey === resetKey ? dateSelection.date : ''
  const asOf = selectedDate
  const widgets = useDashboardWidgetQueries(
    config,
    selectedDate === '' ? null : selectedDate,
  )

  return {
    asOf,
    onAsOfChange: (date) => {
      setDateSelection({ resetKey, date })
    },
    widgets,
  }
}
