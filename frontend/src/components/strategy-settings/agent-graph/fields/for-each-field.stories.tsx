import type { Meta, StoryObj } from '@storybook/react-vite'
import { useState } from 'react'

import { ForEachField } from '#components/strategy-settings/agent-graph/fields/for-each-field'
import type { AgentGraphPhaseForm } from '#components/strategy-settings/agent-graph/types'

const meta = {
  title: 'StrategySettings/AgentGraph/ForEachField',
  component: ForEachField,
} satisfies Meta<typeof ForEachField>

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
      items: { title: { type: 'string' } },
    },
  },
}

const COMPOSE: AgentGraphPhaseForm = {
  key: 'compose',
  label: '節の作成',
  model: 'fictional-model-lantern',
  prompt: '割り当てられた見出しに沿って、説明文を作成する。',
  forEach: 'plan.sections',
  skills: [],
  tools: [],
  output: {},
}

// PhaseCard のフィールドグリッド (grid-cols-(--grid-cols-field-label)) を再現し、実際の見え方に合わせる
function Interactive({
  phases,
  index,
  initial,
  selectDefaultOpen,
}: {
  phases: AgentGraphPhaseForm[]
  index: number
  initial: string | undefined
  selectDefaultOpen?: boolean
}) {
  const [value, setValue] = useState(initial)
  return (
    <div className="grid grid-cols-(--grid-cols-field-label) items-start gap-x-3 gap-y-2 text-xs">
      <ForEachField
        phases={phases}
        index={index}
        value={value}
        onChange={setValue}
        selectDefaultOpen={selectDefaultOpen}
      />
    </div>
  )
}

export const NoPriorArrayOutput: Story = {
  name: 'shows the repeat field when no earlier phase has array output.',
  args: { phases: [PLAN], index: 0, value: undefined, onChange: () => {} },
  render: () => <Interactive phases={[PLAN]} index={0} initial={undefined} />,
}

export const WithArrayOption: Story = {
  name: 'shows available array outputs while choosing items to process.',
  args: {
    phases: [PLAN, COMPOSE],
    index: 1,
    value: undefined,
    onChange: () => {},
  },
  render: () => (
    <Interactive
      phases={[PLAN, COMPOSE]}
      index={1}
      initial={undefined}
      selectDefaultOpen
    />
  ),
}

export const Selected: Story = {
  name: 'shows a selected array output to process for each run.',
  args: {
    phases: [PLAN, COMPOSE],
    index: 1,
    value: 'plan.sections',
    onChange: () => {},
  },
  render: () => (
    <Interactive phases={[PLAN, COMPOSE]} index={1} initial="plan.sections" />
  ),
}
