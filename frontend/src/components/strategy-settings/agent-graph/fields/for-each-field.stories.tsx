import type { Meta, StoryObj } from '@storybook/react-vite'

import { ForEachField } from '#components/strategy-settings/agent-graph/fields/for-each-field'
import type { AgentGraphPhaseForm } from '#components/strategy-settings/agent-graph/types'

const meta = {
  title: 'StrategySettings/AgentGraph/ForEachField',
  component: ForEachField,
} satisfies Meta<typeof ForEachField>

export default meta
type Story = StoryObj<typeof meta>

const ALPHA: AgentGraphPhaseForm = {
  key: 'alpha',
  label: 'サンプル',
  model: 'fictional-model-orbit',
  prompt: 'sample prompt',
  skills: [],
  tools: [],
  output: {
    records: {
      type: 'array',
      items: { name: { type: 'string' } },
    },
  },
}

const BETA: AgentGraphPhaseForm = {
  key: 'beta',
  label: '次の段階',
  model: 'fictional-model-lantern',
  prompt: 'sample prompt',
  forEach: 'alpha.records',
  skills: [],
  tools: [],
  output: {},
}

function renderField(args: Story['args']) {
  return (
    <div className="grid grid-cols-(--grid-cols-field-label) items-start gap-x-3 gap-y-2 text-xs">
      <ForEachField {...args} />
    </div>
  )
}

export const NoPriorArrayOutput: Story = {
  name: 'shows the repeat field when no earlier phase has array output.',
  args: { phases: [ALPHA], index: 0, value: undefined, onChange: () => {} },
  render: renderField,
}

export const WithArrayOption: Story = {
  name: 'shows available array outputs while choosing items to process.',
  args: {
    phases: [ALPHA, BETA],
    index: 1,
    value: undefined,
    onChange: () => {},
  },
  render: renderField,
}

export const Selected: Story = {
  name: 'shows a selected array output to process for each run.',
  args: {
    phases: [ALPHA, BETA],
    index: 1,
    value: 'alpha.records',
    onChange: () => {},
  },
  render: renderField,
}
