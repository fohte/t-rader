import type { Meta, StoryObj } from '@storybook/react-vite'
import { useState } from 'react'

import { LabelFieldField } from '#components/strategy-settings/agent-graph/fields/label-field-field'
import type { AgentGraphPhaseForm } from '#components/strategy-settings/agent-graph/types'

const meta = {
  title: 'StrategySettings/AgentGraph/LabelFieldField',
  component: LabelFieldField,
} satisfies Meta<typeof LabelFieldField>

export default meta
type Story = StoryObj<typeof meta>

const PLAN: AgentGraphPhaseForm = {
  key: 'plan',
  label: '見出し案',
  model: 'fictional-model-orbit',
  prompt: 'テーマを読みやすい節に分ける',
  skills: [],
  tools: [],
  output: {
    sections: {
      type: 'array',
      items: {
        title: { type: 'string' },
        description: { type: 'string' },
      },
    },
  },
}

const PLAN_WITH_PRIMITIVE_ARRAY: AgentGraphPhaseForm = {
  ...PLAN,
  output: {
    key_points: { type: 'array', items: { type: 'string' } },
  },
}

// PhaseCard のフィールドグリッド (grid-cols-(--grid-cols-field-label)) を再現し、実際の見え方に合わせる
function Interactive({
  phases,
  forEach,
  initial,
}: {
  phases: AgentGraphPhaseForm[]
  forEach: string | undefined
  initial: string | undefined
}) {
  const [value, setValue] = useState(initial)
  return (
    <div className="grid grid-cols-(--grid-cols-field-label) items-start gap-x-3 gap-y-2 text-xs">
      <LabelFieldField
        phases={phases}
        forEach={forEach}
        value={value}
        onChange={setValue}
      />
    </div>
  )
}

export const Default: Story = {
  name: 'shows the label field with available output properties.',
  args: {
    phases: [PLAN],
    forEach: 'plan.sections',
    value: undefined,
    onChange: () => {},
  },
  render: () => (
    <Interactive phases={[PLAN]} forEach="plan.sections" initial={undefined} />
  ),
}

export const Selected: Story = {
  name: 'shows a selected label from an earlier phase output.',
  args: {
    phases: [PLAN],
    forEach: 'plan.sections',
    value: 'title',
    onChange: () => {},
  },
  render: () => (
    <Interactive phases={[PLAN]} forEach="plan.sections" initial="title" />
  ),
}

export const StaleValue: Story = {
  name: 'preserves a selected label that is missing from the output properties.',
  args: {
    phases: [PLAN],
    forEach: 'plan.sections',
    value: 'removed_field',
    onChange: () => {},
  },
  render: () => (
    <Interactive
      phases={[PLAN]}
      forEach="plan.sections"
      initial="removed_field"
    />
  ),
}

export const NoOptions: Story = {
  name: 'shows the label field when array items have no named properties.',
  args: {
    phases: [PLAN_WITH_PRIMITIVE_ARRAY],
    forEach: 'plan.key_points',
    value: undefined,
    onChange: () => {},
  },
  render: () => (
    <Interactive
      phases={[PLAN_WITH_PRIMITIVE_ARRAY]}
      forEach="plan.key_points"
      initial={undefined}
    />
  ),
}
