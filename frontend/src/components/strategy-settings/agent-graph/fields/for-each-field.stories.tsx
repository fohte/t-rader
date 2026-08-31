import type { Meta, StoryObj } from '@storybook/react-vite'
import { useState } from 'react'

import { ForEachField } from '#components/strategy-settings/agent-graph/fields/for-each-field'
import type { AgentGraphPhaseForm } from '#components/strategy-settings/agent-graph/types'
import { openSelect } from '#storybook/open-select'

const meta = {
  title: 'StrategySettings/AgentGraph/ForEachField',
  component: ForEachField,
} satisfies Meta<typeof ForEachField>

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
      items: { title: { type: 'string' } },
    },
  },
}

const INVESTIGATE: AgentGraphPhaseForm = {
  key: 'investigate',
  label: '仮説の調査',
  model: 'deepseek-v4-flash',
  prompt: '割り当てられた仮説を検証せよ',
  forEach: 'plan.hypotheses',
  skills: [],
  tools: [],
  output: {},
}

// PhaseCard のフィールドグリッド (grid-cols-(--grid-cols-field-label)) を再現し、実際の見え方に合わせる
function Interactive({
  phases,
  index,
  initial,
}: {
  phases: AgentGraphPhaseForm[]
  index: number
  initial: string | undefined
}) {
  const [value, setValue] = useState(initial)
  return (
    <div className="grid grid-cols-(--grid-cols-field-label) items-start gap-x-3 gap-y-2 text-xs">
      <ForEachField
        phases={phases}
        index={index}
        value={value}
        onChange={setValue}
      />
    </div>
  )
}

export const NoPriorArrayOutput: Story = {
  args: { phases: [PLAN], index: 0, value: undefined, onChange: () => {} },
  render: () => <Interactive phases={[PLAN]} index={0} initial={undefined} />,
}

export const WithArrayOption: Story = {
  args: {
    phases: [PLAN, INVESTIGATE],
    index: 1,
    value: undefined,
    onChange: () => {},
  },
  render: () => (
    <Interactive phases={[PLAN, INVESTIGATE]} index={1} initial={undefined} />
  ),
  play: async ({ canvasElement }) => {
    await openSelect(canvasElement)
  },
}

export const Selected: Story = {
  args: {
    phases: [PLAN, INVESTIGATE],
    index: 1,
    value: 'plan.hypotheses',
    onChange: () => {},
  },
  render: () => (
    <Interactive
      phases={[PLAN, INVESTIGATE]}
      index={1}
      initial="plan.hypotheses"
    />
  ),
}
