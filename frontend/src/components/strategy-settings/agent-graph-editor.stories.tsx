import type { Meta, StoryObj } from '@storybook/react-vite'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import type { Middleware } from 'openapi-fetch'
import type { ReactNode } from 'react'
import { useEffect, useState } from 'react'

import { AgentGraphEditor } from '#components/strategy-settings/agent-graph-editor'
import { fetchClient } from '#lib/api/client'

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
  {
    id: 'fictional-model-maple',
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
// AgentGraphForm (form ビュー) 用にこのファイルの story 全体でモックを用意する
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

function QueryDecorator({ children }: { children: ReactNode }) {
  const [client] = useState(
    () => new QueryClient({ defaultOptions: { queries: { retry: false } } }),
  )
  // 子の useQuery が初回マウント時に fetch する前にモックを登録し切る必要があるため、
  // useEffect ではなく render 中に同期実行される lazy initializer で install する
  const [eject] = useState(() => installMiddleware())
  useEffect(() => eject, [eject])
  return <QueryClientProvider client={client}>{children}</QueryClientProvider>
}

const meta = {
  title: 'StrategySettings/AgentGraphEditor',
  component: AgentGraphEditor,
  decorators: [
    (Story) => (
      <QueryDecorator>
        <Story />
      </QueryDecorator>
    ),
  ],
} satisfies Meta<typeof AgentGraphEditor>

export default meta
type Story = StoryObj<typeof meta>

const SAMPLE = `phases:
  - key: alpha
    label: サンプル
    model: fictional-model-orbit
    runs: once
    tools: [tool_alpha]
    skills: [skill_alpha]
    prompt: sample prompt
    output:
      records:
        type: array
  - key: beta
    label: 次の段階
    model: fictional-model-lantern
    for_each: alpha.records
    label_field: name
    max_parallel: 4
    prompt: sample prompt
  - key: gamma
    label: 最後の段階
    model: fictional-model-maple
    prompt: sample prompt
`

export const FormView: Story = {
  name: 'shows a populated multi-phase agent graph form.',
  args: {
    purpose: 'sample-purpose',
    initialValue: SAMPLE,
    onSave: () => {},
  },
}

export const PhaseSplitDisabled: Story = {
  name: 'shows the phase split toggle switched off with no phase cards.',
  args: {
    purpose: 'sample-purpose',
    initialValue: '',
    onSave: () => {},
  },
}

export const BrokenYaml: Story = {
  name: 'shows a parse error for malformed graph YAML.',
  args: {
    purpose: 'sample-purpose',
    initialValue: 'phases: [',
    onSave: () => {},
  },
}

export const SaveErrorOnPhase: Story = {
  name: 'shows a save error attached to a specific phase.',
  args: {
    purpose: 'sample-purpose',
    initialValue: SAMPLE,
    onSave: () => {},
    saveError: 'phase key "alpha" is duplicated',
  },
}

export const Saving: Story = {
  name: 'shows the graph editor while changes are being saved.',
  args: {
    purpose: 'sample-purpose',
    initialValue: SAMPLE,
    onSave: () => {},
    isSaving: true,
  },
}
