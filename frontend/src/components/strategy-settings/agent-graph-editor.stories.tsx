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
  { name: 'find_reference', description: null },
  { name: 'read_reference', description: null },
  { name: 'save_draft', description: null },
]
const SKILLS = {
  outline_guide: '# outline_guide\n',
  style_guide: '# style_guide\n',
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
  - key: plan
    label: 見出し案
    model: fictional-model-orbit
    runs: once
    tools: [find_reference]
    skills: [outline_guide]
    prompt: 与えられたテーマを 2-4 個の見出しに分ける。
    output:
      sections:
        type: array
  - key: compose
    label: 節の作成
    model: fictional-model-lantern
    for_each: plan.sections
    label_field: title
    max_parallel: 4
    prompt: 割り当てられた見出しに沿って、説明文を作成する。
  - key: synthesize
    label: 統合
    model: fictional-model-maple
    prompt: 作成した節を読みやすい順序にまとめ、全体の概要を記す。
`

export const FormView: Story = {
  name: 'shows a populated multi-phase agent graph form.',
  args: {
    purpose: 'explore',
    initialValue: SAMPLE,
    onSave: () => {},
  },
}

export const PhaseSplitDisabled: Story = {
  name: 'shows the phase split toggle switched off with no phase cards.',
  args: {
    purpose: 'explore',
    initialValue: '',
    onSave: () => {},
  },
}

export const BrokenYaml: Story = {
  name: 'shows a parse error for malformed graph YAML.',
  args: {
    purpose: 'explore',
    initialValue: 'phases: [',
    onSave: () => {},
  },
}

export const SaveErrorOnPhase: Story = {
  name: 'shows a save error attached to a specific phase.',
  args: {
    purpose: 'explore',
    initialValue: SAMPLE,
    onSave: () => {},
    saveError: 'phase key "plan" is duplicated',
  },
}

export const Saving: Story = {
  name: 'shows the graph editor while changes are being saved.',
  args: {
    purpose: 'explore',
    initialValue: SAMPLE,
    onSave: () => {},
    isSaving: true,
  },
}
