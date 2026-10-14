import { createFileRoute, redirect } from '@tanstack/react-router'

import { IndicatorSettingsPageHeader } from '#components/indicators/indicator-settings-page-header'
import { IndicatorsPage } from '#components/indicators/indicators-page'
import { GeneralTab } from '#components/strategy-settings/general-tab'
import {
  StrategySettingsTabs,
  validateStrategySettingsSearch,
} from '#components/strategy-settings/settings-tabs'
import { TriggersTab } from '#components/strategy-settings/triggers-tab'
import { Skeleton } from '#components/ui/skeleton'
import { $api } from '#lib/api/client'

export const Route = createFileRoute('/strategies/$id/settings')({
  validateSearch: validateStrategySettingsSearch,
  beforeLoad: ({ params, search }) => {
    if (search.settingsTab == null) {
      // eslint-disable-next-line @typescript-eslint/only-throw-error, no-restricted-syntax -- TanStack Router の redirect API は throw を要求する
      throw redirect({
        to: '/strategies/$id/settings',
        params: { id: params.id },
        search: { settingsTab: 'general' },
        replace: true,
      })
    }
  },
  component: StrategySettingsPage,
})

function StrategySettingsPage() {
  const { id } = Route.useParams()
  const { settingsTab } = Route.useSearch()
  const tab = settingsTab ?? 'general'

  const { data: strategy, isPending } = $api.useQuery(
    'get',
    '/api/strategies/{id}',
    { params: { path: { id } } },
  )

  if (isPending) {
    return (
      <div className="space-y-4">
        <Skeleton className="h-8 w-72" />
        <Skeleton className="h-80 w-full" />
      </div>
    )
  }

  if (strategy == null) {
    return (
      <div className="font-mono text-sm text-muted-foreground">
        戦略が見つかりませんでした。
      </div>
    )
  }

  return (
    <div className="space-y-5">
      <header>
        <h1 className="mb-1 text-2xl font-bold leading-tight tracking-tight">
          戦略設定 — {strategy.name}
        </h1>
        <p className="text-sm text-muted-foreground-strong">
          基本情報・trigger を編集します。
        </p>
      </header>

      <StrategySettingsTabs strategyId={id} activeTab={tab} />

      <section
        id="strategy-settings-panel"
        role="tabpanel"
        aria-labelledby={`strategy-settings-tab-${tab}`}
      >
        {tab === 'general' && <GeneralTab strategyId={id} />}
        {tab === 'triggers' && <TriggersTab strategyId={id} />}
        {tab === 'indicators' && (
          <div className="space-y-5">
            <IndicatorSettingsPageHeader scope="strategy" />
            <IndicatorsPage scope="strategy" strategyId={id} />
          </div>
        )}
      </section>
    </div>
  )
}
