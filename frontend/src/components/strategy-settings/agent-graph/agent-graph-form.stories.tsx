import type { Meta, StoryObj } from '@storybook/react-vite'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import type { Middleware } from 'openapi-fetch'
import { useEffect, useState } from 'react'

import { AgentGraphForm } from '#components/strategy-settings/agent-graph/agent-graph-form'
import { fetchClient } from '#lib/api/client'

const meta = {
  title: 'StrategySettings/AgentGraph/AgentGraphForm',
  component: AgentGraphForm,
} satisfies Meta<typeof AgentGraphForm>

export default meta
type Story = StoryObj<typeof meta>

const SINGLE_PHASE = `phases:
  - key: alpha
    label: サンプル
    model: fictional-model-orbit
    prompt: sample prompt
    tools: [tool_alpha]
`

const MULTI_PHASE_WITH_FOR_EACH = `phases:
  - key: alpha
    label: サンプル
    model: fictional-model-orbit
    prompt: sample prompt
    tools: [tool_alpha, tool_beta]
    skills: [skill_alpha]
    output:
      records:
        type: array
        items:
          name: { type: string }
          value: { type: string }
  - key: beta
    label: 次の段階
    model: fictional-model-lantern
    for_each: alpha.records
    label_field: name
    prompt: sample prompt
`

const OPERATION_PHASE = `timeout_minutes: 20
execution_limits: { max_model_calls: 6 }
tool_limits: { tool_alpha: 2 }
phases:
  - key: calculate
    label: 計算
    kind: operation
    operation: evaluate_saved_code
    code_id: sample-code
    input:
      request:
        literal: { count: 2 }
      source:
        dataset: { provider: sample-provider, params: { code: DEMO } }
    output:
      result: { type: number }
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
  { name: 'tool_alpha', description: null },
  { name: 'tool_beta', description: null },
  { name: 'tool_gamma', description: null },
]
const SKILLS = {
  skill_alpha: '# skill_alpha\n',
  skill_beta: '# skill_beta\n',
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

function StoryRenderer(args: Story['args']) {
  const [client] = useState(
    () => new QueryClient({ defaultOptions: { queries: { retry: false } } }),
  )
  // 子の useQuery が初回マウント時に fetch する前にモックを登録し切る必要があるため、
  // useEffect ではなく render 中に同期実行される lazy initializer で install する
  const [eject] = useState(() => installMiddleware())
  useEffect(() => eject, [eject])
  return (
    <QueryClientProvider client={client}>
      <AgentGraphForm {...args} />
    </QueryClientProvider>
  )
}

export const ToggleOff: Story = {
  name: 'shows the graph editor switched off with no configured phases.',
  args: {
    purpose: 'sample-purpose',
    value: '',
    onChange: () => {},
    lastEnabledValueRef: { current: '' },
  },
  render: (args) => <StoryRenderer {...args} />,
}

export const ToggleOn: Story = {
  name: 'shows the graph editor switched on with one configured phase.',
  args: {
    purpose: 'sample-purpose',
    value: SINGLE_PHASE,
    onChange: () => {},
    lastEnabledValueRef: { current: SINGLE_PHASE },
  },
  render: (args) => <StoryRenderer {...args} />,
}

export const MultiPhaseWithForEach: Story = {
  name: 'shows a later phase referencing an earlier array field.',
  args: {
    purpose: 'sample-purpose',
    value: MULTI_PHASE_WITH_FOR_EACH,
    onChange: () => {},
    lastEnabledValueRef: { current: MULTI_PHASE_WITH_FOR_EACH },
  },
  render: (args) => <StoryRenderer {...args} />,
}

export const Operation: Story = {
  name: 'shows an operation phase with its input bindings and shared limits.',
  args: {
    purpose: 'sample-purpose',
    value: OPERATION_PHASE,
    onChange: () => {},
    lastEnabledValueRef: { current: OPERATION_PHASE },
  },
  render: (args) => <StoryRenderer {...args} />,
}

export const WithError: Story = {
  name: 'highlights a phase with a configuration error.',
  args: {
    purpose: 'sample-purpose',
    value: MULTI_PHASE_WITH_FOR_EACH,
    onChange: () => {},
    errorPhaseKey: 'beta',
    lastEnabledValueRef: { current: MULTI_PHASE_WITH_FOR_EACH },
  },
  render: (args) => <StoryRenderer {...args} />,
}
