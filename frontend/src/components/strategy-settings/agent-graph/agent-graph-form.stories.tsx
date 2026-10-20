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
    label: 見出し案
    model: fictional-model-orbit
    prompt: 与えられたテーマを読みやすい見出しに分ける。
    tools: [find_reference]
`

const MULTI_PHASE_WITH_FOR_EACH = `phases:
  - key: plan
    label: 見出し案
    model: fictional-model-orbit
    prompt: 与えられたテーマを 2-4 個の見出しに分ける。
    tools: [find_reference, read_reference]
    skills: [outline_guide]
    output:
      sections:
        type: array
        items:
          title: { type: string }
          description: { type: string }
  - key: compose
    label: 節の作成
    model: fictional-model-lantern
    for_each: plan.sections
    label_field: title
    prompt: 割り当てられた見出しに沿って、説明文を作成する。
`

const MODELS = [
  {
    id: 'fictional-model-orbit',
    providers: ['fictional-lab'],
    max_input_tokens: null,
    max_output_tokens: null,
    supports_reasoning: true,
  },
  {
    id: 'fictional-model-lantern',
    providers: ['fictional-lab'],
    max_input_tokens: null,
    max_output_tokens: null,
    supports_reasoning: false,
  },
]
const TOOLS = [
  { name: 'find_reference', description: null },
  { name: 'read_reference', description: null },
  { name: 'save_draft', description: null },
]
const SKILLS = {
  outline_guide: '# outline_guide\n',
  style_guide: '# style_guide\n',
}

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
      if (/\/api\/agent-configs\/[^/]+\/skills(\?|$)/.test(url)) {
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
        purpose="explore"
        value={value}
        onChange={setValue}
        errorPhaseKey={errorPhaseKey}
        lastEnabledValueRef={lastEnabledValueRef}
      />
    </QueryClientProvider>
  )
}

export const ToggleOff: Story = {
  name: 'shows the graph editor switched off with no configured phases.',
  args: {
    purpose: 'explore',
    value: '',
    onChange: () => {},
    lastEnabledValueRef: { current: '' },
  },
  render: () => <Interactive initial="" />,
}

export const ToggleOn: Story = {
  name: 'shows the graph editor switched on with one configured phase.',
  args: {
    purpose: 'explore',
    value: SINGLE_PHASE,
    onChange: () => {},
    lastEnabledValueRef: { current: SINGLE_PHASE },
  },
  render: () => <Interactive initial={SINGLE_PHASE} />,
}

export const MultiPhaseWithForEach: Story = {
  name: 'shows a multi-phase graph that composes each generated section.',
  args: {
    purpose: 'explore',
    value: MULTI_PHASE_WITH_FOR_EACH,
    onChange: () => {},
    lastEnabledValueRef: { current: MULTI_PHASE_WITH_FOR_EACH },
  },
  render: () => <Interactive initial={MULTI_PHASE_WITH_FOR_EACH} />,
}

export const WithError: Story = {
  name: 'highlights a phase with a configuration error.',
  args: {
    purpose: 'explore',
    value: MULTI_PHASE_WITH_FOR_EACH,
    onChange: () => {},
    errorPhaseKey: 'compose',
    lastEnabledValueRef: { current: MULTI_PHASE_WITH_FOR_EACH },
  },
  render: () => (
    <Interactive initial={MULTI_PHASE_WITH_FOR_EACH} errorPhaseKey="compose" />
  ),
}
