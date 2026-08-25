import type { Meta, StoryObj } from '@storybook/react-vite'
import { useState } from 'react'

import { ModelField } from '#components/strategy-settings/agent-graph/fields/model-field'
import type { components } from '#lib/api/schema.gen'

type AgentModel = components['schemas']['AgentModel']

const MODELS: AgentModel[] = [
  {
    id: 'claude-opus-4',
    providers: ['anthropic'],
    max_input_tokens: null,
    max_output_tokens: null,
    supports_reasoning: true,
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
  {
    id: 'deepseek-v4-flash',
    providers: ['deepseek'],
    max_input_tokens: null,
    max_output_tokens: null,
    supports_reasoning: false,
    supports_web_search: false,
  },
]

const meta = {
  title: 'StrategySettings/AgentGraph/ModelField',
  component: ModelField,
} satisfies Meta<typeof ModelField>

export default meta
type Story = StoryObj<typeof meta>

// PhaseCard のフィールドグリッド (grid-cols-(--grid-cols-field-label)) を再現し、実際の見え方に合わせる
function Interactive({
  initial,
  models,
}: {
  initial: string
  models: AgentModel[]
}) {
  const [value, setValue] = useState(initial)
  return (
    <div className="grid grid-cols-(--grid-cols-field-label) items-start gap-x-3 gap-y-2 text-xs">
      <ModelField value={value} onChange={setValue} models={models} />
    </div>
  )
}

export const Default: Story = {
  args: { value: 'claude-sonnet-4', onChange: () => {}, models: MODELS },
  render: () => <Interactive initial="claude-sonnet-4" models={MODELS} />,
}

export const Unset: Story = {
  args: { value: '', onChange: () => {}, models: MODELS },
  render: () => <Interactive initial="" models={MODELS} />,
}

// LiteLLM 未接続などで一覧が引けない場合は自由入力にフォールバックする
export const NoModelsAvailable: Story = {
  args: { value: 'claude-sonnet-4', onChange: () => {}, models: [] },
  render: () => <Interactive initial="claude-sonnet-4" models={[]} />,
}

// 保存済みの値が一覧に無くても (typo・非推奨モデルなど) 選択肢から消えない
export const ValueNotInList: Story = {
  args: { value: 'deprecated-model', onChange: () => {}, models: MODELS },
  render: () => <Interactive initial="deprecated-model" models={MODELS} />,
}
