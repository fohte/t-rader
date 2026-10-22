import { Tabs, TabsList, TabsPanel, TabsTab } from '@fohte/ui/tabs'
import { Link } from '@tanstack/react-router'
import type { ReactNode } from 'react'

import { Skeleton } from '#components/ui/skeleton'

export type AgentConfigTab = 'agents-md' | 'skills' | 'agent-graph'

type AgentConfigPurposeViewProps =
  | { state: 'loading' }
  | { state: 'not-found' }
  | {
      state: 'ready'
      purpose: string
      tab: AgentConfigTab
      onTabChange: (tab: AgentConfigTab) => void
      children: ReactNode
    }

const TABS: { value: AgentConfigTab; label: string }[] = [
  { value: 'agents-md', label: 'AGENTS.md' },
  { value: 'skills', label: 'Skills' },
  { value: 'agent-graph', label: 'Agent' },
]

export function AgentConfigPurposeView(props: AgentConfigPurposeViewProps) {
  if (props.state === 'loading') {
    return (
      <div className="space-y-4">
        <Skeleton className="h-8 w-72" />
        <Skeleton className="h-80 w-full" />
      </div>
    )
  }

  if (props.state === 'not-found') {
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
          Agent 設定 — {props.purpose}
        </h1>
        <p className="text-sm text-muted-foreground-strong">
          AGENTS.md / skills / agent graph を編集します。
        </p>
      </header>

      <Tabs
        value={props.tab}
        onValueChange={(value) => {
          const nextTab = TABS.find((tab) => tab.value === value)
          if (nextTab != null) props.onTabChange(nextTab.value)
        }}
      >
        <TabsList aria-label="Agent 設定タブ">
          {TABS.map((tab) => (
            <TabsTab key={tab.value} value={tab.value}>
              {tab.label}
            </TabsTab>
          ))}
        </TabsList>
        <TabsPanel value={props.tab}>{props.children}</TabsPanel>
      </Tabs>
    </div>
  )
}
