import { createFileRoute, Link } from '@tanstack/react-router'
import { useState } from 'react'

import { AgentsMdTab } from '#components/strategy-settings/agents-md-tab'
import { SkillsTab } from '#components/strategy-settings/skills-tab'
import { TriggersTab } from '#components/strategy-settings/triggers-tab'
import { Skeleton } from '#components/ui/skeleton'
import { $api } from '#lib/api/client'

type TabKey = 'agents-md' | 'skills' | 'triggers'

const TABS: { key: TabKey; label: string }[] = [
  { key: 'agents-md', label: 'AGENTS.md' },
  { key: 'skills', label: 'skills' },
  { key: 'triggers', label: 'triggers' },
]

export const Route = createFileRoute('/strategies/$id/settings')({
  component: StrategySettingsPage,
})

function StrategySettingsPage() {
  const { id } = Route.useParams()
  const [tab, setTab] = useState<TabKey>('agents-md')

  const { data: strategy, isPending } = $api.useQuery(
    'get',
    '/api/strategies/{id}',
    { params: { path: { id } } },
  )

  if (isPending) {
    return (
      <div className="space-y-4">
        <Skeleton className="h-8 w-72" />
        <Skeleton className="h-[320px] w-full" />
      </div>
    )
  }

  if (strategy == null) {
    return (
      <div className="font-mono text-[13px] text-[color:var(--color-text-tertiary)]">
        戦略が見つかりませんでした。
      </div>
    )
  }

  return (
    <div className="space-y-5">
      <div>
        <Link
          to="/strategies/$id"
          params={{ id }}
          className="font-mono text-[12px] text-[color:var(--color-text-tertiary)] hover:text-[color:var(--color-text-primary)]"
        >
          &lt; {strategy.name} に戻る
        </Link>
      </div>
      <header>
        <h1 className="mb-1 text-[24px] font-bold leading-tight tracking-tight">
          戦略設定 — {strategy.name}
        </h1>
        <p className="text-[13px] text-[color:var(--color-text-secondary)]">
          戦略 Agent の初期コンテキスト (AGENTS.md / skills) や trigger
          を編集します。
        </p>
      </header>

      <div
        role="tablist"
        aria-label="戦略設定タブ"
        className="flex items-center gap-1 border-b border-[color:var(--color-hairline)]"
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
              className="border-b-2 px-3 py-1.5 font-mono text-[12px] uppercase tracking-wider data-[active=true]:border-[color:var(--color-accent-strategy)] data-[active=true]:text-[color:var(--color-accent-strategy)] data-[active=false]:border-transparent data-[active=false]:text-[color:var(--color-text-tertiary)]"
              data-active={active}
            >
              {t.label}
            </button>
          )
        })}
      </div>

      <section role="tabpanel">
        {tab === 'agents-md' && <AgentsMdTab strategyId={id} />}
        {tab === 'skills' && <SkillsTab strategyId={id} />}
        {tab === 'triggers' && <TriggersTab strategyId={id} />}
      </section>
    </div>
  )
}
