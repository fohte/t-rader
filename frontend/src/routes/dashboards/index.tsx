import { createFileRoute } from '@tanstack/react-router'

import { parseDashboardConfig } from '#components/dashboard-pages/dashboard-config'
import {
  type DashboardListPageState,
  DashboardListPageView,
} from '#components/dashboard-pages/dashboard-list-page-view'
import { $api } from '#lib/api/client'
import { getApiErrorMessage } from '#lib/api/error'

export const Route = createFileRoute('/dashboards/')({
  component: DashboardListRoute,
})

function DashboardListRoute() {
  const dashboardsQuery = $api.useQuery('get', '/api/dashboards')
  let state: DashboardListPageState

  if (dashboardsQuery.isPending) {
    state = { status: 'loading' }
  } else if (dashboardsQuery.isError) {
    state = {
      status: 'error',
      message: getApiErrorMessage(
        dashboardsQuery.error,
        'ダッシュボードの一覧を取得できませんでした。',
      ),
    }
  } else {
    state = {
      status: 'ready',
      dashboards: dashboardsQuery.data.map((dashboard) => {
        const parsed = parseDashboardConfig(dashboard.config)
        return {
          key: dashboard.key,
          title: parsed.ok ? parsed.value.title : dashboard.key,
        }
      }),
    }
  }

  return <DashboardListPageView state={state} />
}
