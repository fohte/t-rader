import { useState } from 'react'

import {
  type DashboardConfig,
  getInitialDashboardDate,
} from '#components/dashboard-pages/dashboard-config'
import type { DashboardWidgetPageState } from '#components/dashboard-pages/dashboard-page-view'
import { useDashboardWidgetQueries } from '#components/dashboard-pages/use-dashboard-widget-queries'

type DashboardDateSelection = {
  resetKey: string
  date: string
}

export function useDashboardPageData(
  config: DashboardConfig | null,
  resetKey: string,
  fallbackDate: string,
): {
  asOf: string
  onAsOfChange: (date: string) => void
  widgets: DashboardWidgetPageState[]
} {
  const initialDate =
    config == null ? null : getInitialDashboardDate(config, fallbackDate)
  const [dateSelection, setDateSelection] =
    useState<DashboardDateSelection | null>(null)
  const selectedDate =
    dateSelection?.resetKey === resetKey ? dateSelection.date : null
  const asOf = selectedDate ?? initialDate ?? ''
  const widgets = useDashboardWidgetQueries(config, selectedDate ?? initialDate)

  return {
    asOf,
    onAsOfChange: (date) => {
      setDateSelection(date === '' ? null : { resetKey, date })
    },
    widgets,
  }
}
