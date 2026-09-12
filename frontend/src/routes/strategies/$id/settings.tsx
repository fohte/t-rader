import { createFileRoute, Link } from '@tanstack/react-router'
import { useState } from 'react'

import { InterestTree } from '#components/strategy-home/interest-tree'
import { GeneralTab } from '#components/strategy-settings/general-tab'
import { RiskPolicyTab } from '#components/strategy-settings/risk-policy-tab'
import { TriggersTab } from '#components/strategy-settings/triggers-tab'
import { Skeleton } from '#components/ui/skeleton'
import { $api } from '#lib/api/client'

type TabKey = 'general' | 'triggers' | 'risk-policy' | 'interests'

const TABS: { key: TabKey; label: string }[] = [
  { key: 'general', label: '全般' },
  { key: 'triggers', label: 'Triggers' },
  { key: 'risk-policy', label: 'リスク上限' },
  { key: 'interests', label: '関心' },
]

export const Route = createFileRoute('/strategies/$id/settings')({
  component: StrategySettingsPage,
})

function StrategySettingsPage() {
  const { id } = Route.useParams()
  const [tab, setTab] = useState<TabKey>('general')

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
      <div>
        <Link
          to="/strategies"
          className="font-mono text-xs text-muted-foreground hover:text-foreground"
        >
          &lt; 戦略一覧に戻る
        </Link>
      </div>
      <header>
        <h1 className="mb-1 text-2xl font-bold leading-tight tracking-tight">
          戦略設定 — {strategy.name}
        </h1>
        <p className="text-sm text-muted-foreground-strong">
          基本情報・trigger・リスク上限を編集します。
        </p>
      </header>

      <div
        role="tablist"
        aria-label="戦略設定タブ"
        className="flex items-center gap-1 border-b border-border"
      >
        {TABS.map((t) => {
          const active = tab === t.key
          return (
            <button
              key={t.key}
              role="tab"
              type="button"
              aria-selected={active}
              onClick={() => {
                setTab(t.key)
              }}
              className="border-b-2 px-3 py-1.5 font-mono text-xs uppercase tracking-wider data-[active=true]:border-primary data-[active=true]:text-primary data-[active=false]:border-transparent data-[active=false]:text-muted-foreground"
              data-active={active}
            >
              {t.label}
            </button>
          )
        })}
      </div>

      <section role="tabpanel">
        {tab === 'general' && <GeneralTab strategyId={id} />}
        {tab === 'triggers' && <TriggersTab strategyId={id} />}
        {tab === 'risk-policy' && <RiskPolicyTab strategyId={id} />}
        {tab === 'interests' && <InterestTree strategyId={id} />}
      </section>
    </div>
  )
}
