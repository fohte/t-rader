import { createFileRoute, Outlet } from '@tanstack/react-router'

import { AssetTabs } from '#components/assets/asset-tabs'

export const Route = createFileRoute('/trades')({
  component: TradesLayout,
})

function TradesLayout() {
  return (
    <div className="space-y-5 font-sans text-foreground">
      <AssetTabs activeTab="trades" />
      <Outlet />
    </div>
  )
}
