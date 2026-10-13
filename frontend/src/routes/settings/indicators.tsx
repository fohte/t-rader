import { createFileRoute } from '@tanstack/react-router'

import { IndicatorSettingsPageHeader } from '#components/indicators/indicator-settings-page-header'
import { IndicatorsPage } from '#components/indicators/indicators-page'

export const Route = createFileRoute('/settings/indicators')({
  component: GlobalIndicatorsSettingsPage,
})

function GlobalIndicatorsSettingsPage() {
  return (
    <div className="space-y-5">
      <IndicatorSettingsPageHeader scope="global" />
      <IndicatorsPage scope="global" showHeader={false} />
    </div>
  )
}
