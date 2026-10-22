import type { Meta, StoryObj } from '@storybook/react-vite'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import type { Middleware } from 'openapi-fetch'
import { useEffect, useState } from 'react'

import { TriggersTab } from '#components/strategy-settings/triggers-tab'
import { fetchClient } from '#lib/api/client'
import type { components } from '#lib/api/schema.gen'

type Trigger = components['schemas']['Trigger']

function installMiddleware(initialTriggers: Trigger[]) {
  const middleware: Middleware = {
    onRequest({ request }) {
      if (
        request.method === 'GET' &&
        /\/api\/strategies\/demo-strategy\/triggers(?:\?|$)/.test(request.url)
      ) {
        return new Response(JSON.stringify(initialTriggers), {
          status: 200,
          headers: { 'content-type': 'application/json' },
        })
      }
      if (
        request.method === 'GET' &&
        /\/api\/agent-configs(?:\?|$)/.test(request.url)
      ) {
        return new Response(
          JSON.stringify([{ purpose: 'synthetic-purpose' }]),
          {
            status: 200,
            headers: { 'content-type': 'application/json' },
          },
        )
      }
      return new Response('unmocked story request', { status: 404 })
    },
  }
  fetchClient.use(middleware)
  return () => {
    fetchClient.eject(middleware)
  }
}

function TriggersStory({ initialTriggers }: { initialTriggers: Trigger[] }) {
  const [client] = useState(
    () => new QueryClient({ defaultOptions: { queries: { retry: false } } }),
  )
  const [eject] = useState(() => installMiddleware(initialTriggers))
  useEffect(() => eject, [eject])

  return (
    <QueryClientProvider client={client}>
      <div className="p-6">
        <TriggersTab strategyId="demo-strategy" />
      </div>
    </QueryClientProvider>
  )
}

const meta = {
  title: 'StrategySettings/TriggersTab',
} satisfies Meta

export default meta
type Story = StoryObj<typeof meta>

const SAMPLE_TRIGGER: Trigger = {
  trigger_id: 'demo-trigger',
  strategy_id: 'demo-strategy',
  purpose: 'synthetic-purpose',
  kind: 'cron',
  schedule: '15 10 * * 2',
  hook_slug: null,
  event_type: null,
  event_match: null,
  prompt_template: 'synthetic prompt',
  enabled: true,
  business_days_only: false,
  last_fired_at: null,
  created_at: '2000-01-01T00:00:00Z',
  updated_at: '2000-01-01T00:00:00Z',
}

export const SelectedTrigger: Story = {
  name: 'the tab shows a selected trigger and its form.',
  render: () => <TriggersStory initialTriggers={[SAMPLE_TRIGGER]} />,
}

export const Empty: Story = {
  name: 'the tab explains when no triggers exist.',
  render: () => <TriggersStory initialTriggers={[]} />,
}
