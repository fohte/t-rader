import { createFileRoute, Link } from '@tanstack/react-router'
import { useState } from 'react'

import { AgentGraphTab } from '#components/strategy-settings/agent-graph-tab'
import { AgentsMdTab } from '#components/strategy-settings/agents-md-tab'
import { SkillsTab } from '#components/strategy-settings/skills-tab'
import { TriggersTab } from '#components/strategy-settings/triggers-tab'
import { Skeleton } from '#components/ui/skeleton'
import { $api } from '#lib/api/client'

type TabKey = 'agents-md' | 'skills' | 'triggers' | 'agent-graph'

const TABS: { key: TabKey; label: string }[] = [
  { key: 'agents-md', label: 'AGENTS.md' },
  { key: 'skills', label: 'Skills' },
  { key: 'agent-graph', label: 'Agent' },
  { key: 'triggers', label: 'Triggers' },
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
          to="/strategies/$id"
          params={{ id }}
          className="font-mono text-xs text-muted-foreground hover:text-foreground"
        >
          &lt; {strategy.name} に戻る
        </Link>
      </div>
      <header>
        <h1 className="mb-1 text-2xl font-bold leading-tight tracking-tight">
          戦略設定 — {strategy.name}
        </h1>
        <p className="text-sm text-muted-foreground-strong">
          戦略 Agent の初期コンテキスト (AGENTS.md / skills)、trigger、 agent
          graph を編集します。
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
        {tab === 'agents-md' && <AgentsMdTab strategyId={id} />}
        {tab === 'skills' && <SkillsTab strategyId={id} />}
        {tab === 'agent-graph' && <AgentGraphTab strategyId={id} />}
        {tab === 'triggers' && <TriggersTab strategyId={id} />}
      </section>
    </div>
  )
}
