import type { Meta, StoryObj } from '@storybook/react-vite'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import type { Middleware } from 'openapi-fetch'
import { useEffect, useRef, useState } from 'react'

import { AgentGraphForm } from '#components/strategy-settings/agent-graph/agent-graph-form'
import { fetchClient } from '#lib/api/client'

const meta = {
  title: 'StrategySettings/AgentGraph/AgentGraphForm',
  component: AgentGraphForm,
} satisfies Meta<typeof AgentGraphForm>

export default meta
type Story = StoryObj<typeof meta>

const SINGLE_PHASE = `phases:
  - key: plan
    label: 調査計画
    model: claude-opus-4
    prompt: 与えられた問いに対し、検証すべき仮説を立てよ。
    tools: [list_notes]
`

const MULTI_PHASE_WITH_FOR_EACH = `phases:
  - key: plan
    label: 調査計画
    model: claude-opus-4
    prompt: 与えられた問いに対し、検証すべき仮説を 2-4 件立てよ。
    tools: [list_notes, query_data]
    skills: [snapshot]
    output:
      hypotheses:
        type: array
  - key: investigate
    label: 仮説の調査
    model: deepseek-v4-flash
    for_each: plan.hypotheses
    label_field: title
    prompt: 割り当てられた 1 件だけを検証し、結論をノートに書け。
`

const MODELS = [
  {
    id: 'claude-opus-4',
    providers: ['anthropic'],
    max_input_tokens: null,
    max_output_tokens: null,
    supports_reasoning: true,
    supports_web_search: false,
  },
  {
    id: 'deepseek-v4-flash',
    providers: ['deepseek'],
    max_input_tokens: null,
    max_output_tokens: null,
    supports_reasoning: false,
    supports_web_search: false,
  },
]
const TOOLS = [
  { name: 'list_notes', description: null },
  { name: 'query_data', description: null },
  { name: 'write_note', description: null },
]
const SKILLS = { snapshot: '# snapshot\n', recap: '# recap\n' }

// Storybook にはグローバルな QueryClientProvider が無いため、$api.useQuery を使う
// AgentGraphForm 用に各 story でモックを用意する
function installMiddleware() {
  const middleware: Middleware = {
    onRequest({ request }) {
      const { url } = request
      if (/\/api\/agent-models(\?|$)/.test(url)) {
        return new Response(JSON.stringify({ models: MODELS }), {
          status: 200,
          headers: { 'content-type': 'application/json' },
        })
      }
      if (/\/api\/agent-tools(\?|$)/.test(url)) {
        return new Response(JSON.stringify({ tools: TOOLS }), {
          status: 200,
          headers: { 'content-type': 'application/json' },
        })
      }
      if (/\/api\/strategies\/[^/]+\/skills(\?|$)/.test(url)) {
        return new Response(JSON.stringify({ skills: SKILLS }), {
          status: 200,
          headers: { 'content-type': 'application/json' },
        })
      }
      return new Response(`unmocked request: ${request.method} ${url}`, {
        status: 404,
      })
    },
  }
  fetchClient.use(middleware)
  return () => {
    fetchClient.eject(middleware)
  }
}

function Interactive({
  initial,
  errorPhaseKey = null,
}: {
  initial: string
  errorPhaseKey?: string | null
}) {
  const [value, setValue] = useState(initial)
  const lastEnabledValueRef = useRef(value)
  useEffect(() => {
    if (value.trim() !== '') lastEnabledValueRef.current = value
  }, [value])
  const [client] = useState(
    () => new QueryClient({ defaultOptions: { queries: { retry: false } } }),
  )
  // 子の useQuery が初回マウント時に fetch する前にモックを登録し切る必要があるため、
  // useEffect ではなく render 中に同期実行される lazy initializer で install する
  const [eject] = useState(() => installMiddleware())
  useEffect(() => eject, [eject])
  return (
    <QueryClientProvider client={client}>
      <AgentGraphForm
        strategyId="strat-1"
        value={value}
        onChange={setValue}
        errorPhaseKey={errorPhaseKey}
        lastEnabledValueRef={lastEnabledValueRef}
      />
    </QueryClientProvider>
  )
}

export const ToggleOff: Story = {
  args: {
    strategyId: 'strat-1',
    value: '',
    onChange: () => {},
    lastEnabledValueRef: { current: '' },
  },
  render: () => <Interactive initial="" />,
}

export const ToggleOn: Story = {
  args: {
    strategyId: 'strat-1',
    value: SINGLE_PHASE,
    onChange: () => {},
    lastEnabledValueRef: { current: SINGLE_PHASE },
  },
  render: () => <Interactive initial={SINGLE_PHASE} />,
}

export const MultiPhaseWithForEach: Story = {
  args: {
    strategyId: 'strat-1',
    value: MULTI_PHASE_WITH_FOR_EACH,
    onChange: () => {},
    lastEnabledValueRef: { current: MULTI_PHASE_WITH_FOR_EACH },
  },
  render: () => <Interactive initial={MULTI_PHASE_WITH_FOR_EACH} />,
}

export const WithError: Story = {
  args: {
    strategyId: 'strat-1',
    value: MULTI_PHASE_WITH_FOR_EACH,
    onChange: () => {},
    errorPhaseKey: 'investigate',
    lastEnabledValueRef: { current: MULTI_PHASE_WITH_FOR_EACH },
  },
  render: () => (
    <Interactive
      initial={MULTI_PHASE_WITH_FOR_EACH}
      errorPhaseKey="investigate"
    />
  ),
}
