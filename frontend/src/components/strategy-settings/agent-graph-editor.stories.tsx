import type { Meta, StoryObj } from '@storybook/react-vite'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import type { Middleware } from 'openapi-fetch'
import type { ReactNode } from 'react'
import { useEffect, useState } from 'react'

import { AgentGraphEditor } from '#components/strategy-settings/agent-graph-editor'
import { fetchClient } from '#lib/api/client'

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
  {
    id: 'claude-sonnet-4',
    providers: ['anthropic'],
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
    label: 調査計画
    model: claude-opus-4
    runs: once
    tools: [list_notes]
    skills: [snapshot]
    prompt: 与えられた問いに対し、検証すべき仮説を 2-4 件立てよ。
    output:
      hypotheses:
        type: array
  - key: investigate
    label: 仮説の調査
    model: deepseek-v4-flash
    for_each: plan.hypotheses
    label_field: title
    max_parallel: 4
    prompt: 割り当てられた 1 件だけを検証し、結論をノートに書け。
  - key: synthesize
    label: 統合
    model: claude-sonnet-4
    prompt: 調査結果をまとめ、結論のノートを 1 本書け。
`

export const FormView: Story = {
  args: {
    strategyId: 'strat-1',
    initialValue: SAMPLE,
    onSave: () => {},
  },
}

export const PhaseSplitDisabled: Story = {
  args: {
    strategyId: 'strat-1',
    initialValue: '',
    onSave: () => {},
  },
}

export const BrokenYaml: Story = {
  args: {
    strategyId: 'strat-1',
    initialValue: 'phases: [',
    onSave: () => {},
  },
}

export const SaveErrorOnPhase: Story = {
  args: {
    strategyId: 'strat-1',
    initialValue: SAMPLE,
    onSave: () => {},
    saveError: 'phase key "plan" is duplicated',
  },
}

export const Saving: Story = {
  args: {
    strategyId: 'strat-1',
    initialValue: SAMPLE,
    onSave: () => {},
    isSaving: true,
  },
}
