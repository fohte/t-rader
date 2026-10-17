import { createFileRoute } from '@tanstack/react-router'
import { useMemo } from 'react'

import { getTokyoDate } from '#components/calendar/calendar-date'
import { parseDashboardConfig } from '#components/dashboard-pages/dashboard-config'
import { DashboardPageView } from '#components/dashboard-pages/dashboard-page-view'
import { useDashboardPageData } from '#components/dashboard-pages/use-dashboard-page-data'
import { $api } from '#lib/api/client'
import { getApiErrorMessage, getApiErrorStatus } from '#lib/api/error'

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
  const defaultDate = useMemo(() => getTokyoDate(), [])
  const config = parsedConfig?.ok === true ? parsedConfig.value : null
  const dashboardData = useDashboardPageData(config, key, defaultDate)

  if (dashboardQuery.isPending) {
    return <DashboardPageView state={{ status: 'loading', title: key }} />
  }
  if (dashboardQuery.isError) {
    if (getApiErrorStatus(dashboardQuery.error) === 404) {
      return <DashboardPageView state={{ status: 'not-found', title: key }} />
    }
    return (
      <DashboardPageView
        state={{
          status: 'error',
          title: key,
          message: getApiErrorMessage(
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
        ...dashboardData,
      }}
    />
  )
}
