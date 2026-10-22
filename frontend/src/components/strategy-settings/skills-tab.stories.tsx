import type { Meta, StoryObj } from '@storybook/react-vite'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import type { Middleware } from 'openapi-fetch'
import { useEffect, useState } from 'react'

import { SkillsTab } from '#components/strategy-settings/skills-tab'
import { fetchClient } from '#lib/api/client'

type SkillStore = Record<string, string>

function installMiddleware(initialSkills: SkillStore) {
  const skills = { ...initialSkills }
  const middleware: Middleware = {
    onRequest({ request }) {
      if (
        request.method === 'GET' &&
        /\/api\/agent-configs\/[^/]+\/skills(?:\?|$)/.test(request.url)
      ) {
        return new Response(JSON.stringify({ skills }), {
          status: 200,
          headers: { 'content-type': 'application/json' },
        })
      }
      return new Response('unmocked story request', { status: 404 })
    },
  }
  fetchClient.use(middleware)
  return () => {
    fetchClient.eject(middleware)
  }
}

function SkillsStory({ initialSkills }: { initialSkills: SkillStore }) {
  const [client] = useState(
    () => new QueryClient({ defaultOptions: { queries: { retry: false } } }),
  )
  const [eject] = useState(() => installMiddleware(initialSkills))
  useEffect(() => eject, [eject])

  return (
    <QueryClientProvider client={client}>
      <div className="p-6">
        <SkillsTab purpose="synthetic-purpose" />
      </div>
    </QueryClientProvider>
  )
}

const meta = {
  title: 'StrategySettings/SkillsTab',
} satisfies Meta

export default meta
type Story = StoryObj<typeof meta>

export const SelectedSkill: Story = {
  name: 'the tab shows a selected skill and its editor.',
  render: () => (
    <SkillsStory
      initialSkills={{
        'demo-helper': '# Demo guide\n\nSample content for the editor.',
        'sample-guide': '# Sample guide\n\nAnother fictional example.',
      }}
    />
  ),
}

export const Empty: Story = {
  name: 'the tab explains when no skills are available.',
  render: () => <SkillsStory initialSkills={{}} />,
}
