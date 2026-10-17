import { createFileRoute } from '@tanstack/react-router'
import { useMemo, useState } from 'react'

import { getTokyoDate } from '#components/calendar/calendar-date'
import {
  getDashboardApiErrorMessage,
  getDashboardApiErrorStatus,
  getInitialDashboardDate,
  parseDashboardConfig,
} from '#components/dashboard-pages/dashboard-config'
import { DashboardPageView } from '#components/dashboard-pages/dashboard-page-view'
import { useDashboardWidgetQueries } from '#components/dashboard-pages/use-dashboard-widget-queries'
import { $api } from '#lib/api/client'

export const Route = createFileRoute('/dashboards/$key')({
  component: DashboardDetailRoute,
})

function DashboardDetailRoute() {
  const { key } = Route.useParams()
  const dashboardQuery = $api.useQuery('get', '/api/dashboards/{key}', {
    params: { path: { key } },
  })
  const parsedConfig = useMemo(
    () =>
      dashboardQuery.data == null
        ? null
        : parseDashboardConfig(dashboardQuery.data.config),
    [dashboardQuery.data?.config],
  )
  const [dateOverride, setDateOverride] = useState<{
    key: string
    date: string
  } | null>(null)
  const defaultDate = useMemo(() => getTokyoDate(), [])
  const config = parsedConfig?.ok === true ? parsedConfig.value : null
  const asOf =
    dateOverride?.key === key
      ? dateOverride.date
      : config == null
        ? defaultDate
        : getInitialDashboardDate(config, defaultDate)
  const widgets = useDashboardWidgetQueries(config, asOf)

  if (dashboardQuery.isPending) {
    return <DashboardPageView state={{ status: 'loading', title: key }} />
  }
  if (dashboardQuery.isError) {
    if (getDashboardApiErrorStatus(dashboardQuery.error) === 404) {
      return <DashboardPageView state={{ status: 'not-found', title: key }} />
    }
    return (
      <DashboardPageView
        state={{
          status: 'error',
          title: key,
          message: getDashboardApiErrorMessage(
            dashboardQuery.error,
            'ダッシュボードを取得できませんでした。',
          ),
        }}
      />
    )
  }
  if (parsedConfig == null || !parsedConfig.ok) {
    return (
      <DashboardPageView
        state={{
          status: 'invalid',
          title: key,
          issues: parsedConfig?.issues ?? [
            {
              path: '$',
              message: 'ダッシュボード設定を読み込めませんでした。',
            },
          ],
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
          if (date !== '') setDateOverride({ key, date })
        },
        widgets,
      }}
    />
  )
}
