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
  label: '調査計画',
  model: 'claude-opus-4',
  prompt: '仮説を立てよ',
  skills: [],
  tools: [],
  output: {
    hypotheses: {
      type: 'array',
      items: {
        title: { type: 'string' },
        rationale: { type: 'string' },
      },
    },
  },
}

const PLAN_WITH_PRIMITIVE_ARRAY: AgentGraphPhaseForm = {
  ...PLAN,
  output: {
    checks: { type: 'array', items: { type: 'string' } },
  },
}

// PhaseCard のフィールドグリッド (grid-cols-[108px_1fr]) を再現し、実際の見え方に合わせる
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
    <div className="grid grid-cols-[108px_1fr] items-start gap-x-3 gap-y-2 text-[12px]">
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
  args: {
    phases: [PLAN],
    forEach: 'plan.hypotheses',
    value: undefined,
    onChange: () => {},
  },
  render: () => (
    <Interactive
      phases={[PLAN]}
      forEach="plan.hypotheses"
      initial={undefined}
    />
  ),
}

export const Selected: Story = {
  args: {
    phases: [PLAN],
    forEach: 'plan.hypotheses',
    value: 'title',
    onChange: () => {},
  },
  render: () => (
    <Interactive phases={[PLAN]} forEach="plan.hypotheses" initial="title" />
  ),
}

export const StaleValue: Story = {
  args: {
    phases: [PLAN],
    forEach: 'plan.hypotheses',
    value: 'removed_field',
    onChange: () => {},
  },
  render: () => (
    <Interactive
      phases={[PLAN]}
      forEach="plan.hypotheses"
      initial="removed_field"
    />
  ),
}

export const NoOptions: Story = {
  args: {
    phases: [PLAN_WITH_PRIMITIVE_ARRAY],
    forEach: 'plan.checks',
    value: undefined,
    onChange: () => {},
  },
  render: () => (
    <Interactive
      phases={[PLAN_WITH_PRIMITIVE_ARRAY]}
      forEach="plan.checks"
      initial={undefined}
    />
  ),
}
