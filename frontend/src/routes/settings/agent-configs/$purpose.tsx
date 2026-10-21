import { createFileRoute } from '@tanstack/react-router'
import { useState } from 'react'

import {
  AgentConfigPurposeView,
  type AgentConfigTab,
} from '#components/strategy-settings/agent-configs-purpose-view'
import { AgentGraphTab } from '#components/strategy-settings/agent-graph-tab'
import { AgentsMdTab } from '#components/strategy-settings/agents-md-tab'
import { SkillsTab } from '#components/strategy-settings/skills-tab'
import { $api } from '#lib/api/client'

export const Route = createFileRoute('/settings/agent-configs/$purpose')({
  component: AgentConfigSettingsPage,
})

function AgentConfigSettingsPage() {
  const { purpose } = Route.useParams()
  const [tab, setTab] = useState<AgentConfigTab>('agents-md')

  const { data: agentConfig, isPending } = $api.useQuery(
    'get',
    '/api/agent-configs/{purpose}',
    { params: { path: { purpose } } },
  )

  if (isPending) {
    return <AgentConfigPurposeView state="loading" />
  }

  if (agentConfig == null) {
    return <AgentConfigPurposeView state="not-found" />
  }

  return (
    <AgentConfigPurposeView
      state="ready"
      purpose={purpose}
      tab={tab}
      onTabChange={setTab}
    >
      {tab === 'agents-md' && <AgentsMdTab purpose={purpose} />}
      {tab === 'skills' && <SkillsTab purpose={purpose} />}
      {tab === 'agent-graph' && <AgentGraphTab purpose={purpose} />}
    </AgentConfigPurposeView>
  )
}
