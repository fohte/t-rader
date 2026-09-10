import { createFileRoute, Link } from '@tanstack/react-router'
import { useState } from 'react'

import { AgentGraphTab } from '#components/strategy-settings/agent-graph-tab'
import { AgentsMdTab } from '#components/strategy-settings/agents-md-tab'
import { SkillsTab } from '#components/strategy-settings/skills-tab'
import { Skeleton } from '#components/ui/skeleton'
import { $api } from '#lib/api/client'

type TabKey = 'agents-md' | 'skills' | 'agent-graph'

const TABS: { key: TabKey; label: string }[] = [
  { key: 'agents-md', label: 'AGENTS.md' },
  { key: 'skills', label: 'Skills' },
  { key: 'agent-graph', label: 'Agent' },
]

export const Route = createFileRoute('/settings/agent-configs/$purpose')({
  component: AgentConfigSettingsPage,
})

function AgentConfigSettingsPage() {
  const { purpose } = Route.useParams()
  const [tab, setTab] = useState<TabKey>('agents-md')

  const { data: agentConfig, isPending } = $api.useQuery(
    'get',
    '/api/agent-configs/{purpose}',
    { params: { path: { purpose } } },
  )

  if (isPending) {
    return (
      <div className="space-y-4">
        <Skeleton className="h-8 w-72" />
        <Skeleton className="h-80 w-full" />
      </div>
    )
  }

  if (agentConfig == null) {
    return (
      <div className="font-mono text-sm text-muted-foreground">
        Agent 設定が見つかりませんでした。
      </div>
    )
  }

  return (
    <div className="space-y-5">
      <div>
        <Link
          to="/settings/agent-configs"
          className="font-mono text-xs text-muted-foreground hover:text-foreground"
        >
          &lt; Agent 設定に戻る
        </Link>
      </div>
      <header>
        <h1 className="mb-1 text-2xl font-bold leading-tight tracking-tight">
          Agent 設定 — {purpose}
        </h1>
        <p className="text-sm text-muted-foreground-strong">
          AGENTS.md / skills / agent graph を編集します。
        </p>
      </header>

      <div
        role="tablist"
        aria-label="Agent 設定タブ"
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
        {tab === 'agents-md' && <AgentsMdTab purpose={purpose} />}
        {tab === 'skills' && <SkillsTab purpose={purpose} />}
        {tab === 'agent-graph' && <AgentGraphTab purpose={purpose} />}
      </section>
    </div>
  )
}
