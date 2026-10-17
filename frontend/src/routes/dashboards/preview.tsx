import { createFileRoute, useLocation } from '@tanstack/react-router'
import { useMemo } from 'react'

import { parseDashboardPreviewHash } from '#components/dashboard-pages/dashboard-config'
import { DashboardPageView } from '#components/dashboard-pages/dashboard-page-view'
import { useDashboardPageData } from '#components/dashboard-pages/use-dashboard-page-data'

export const Route = createFileRoute('/dashboards/preview')({
  component: DashboardPreviewRoute,
})

function DashboardPreviewRoute() {
  const hash = useLocation({ select: (location) => location.hash })
  const parsedConfig = useMemo(() => parseDashboardPreviewHash(hash), [hash])
  const config = parsedConfig.ok ? parsedConfig.value : null
  const dashboardData = useDashboardPageData(config, hash)

  if (!parsedConfig.ok) {
    return (
      <DashboardPageView
        showDashboardListLink={false}
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
      showDashboardListLink={false}
      state={{
        status: 'ready',
        title: parsedConfig.value.title,
        ...dashboardData,
      }}
    />
  )
}
