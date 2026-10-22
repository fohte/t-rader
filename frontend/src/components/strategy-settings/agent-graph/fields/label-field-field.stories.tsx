import type { Meta, StoryObj } from '@storybook/react-vite'

import { LabelFieldField } from '#components/strategy-settings/agent-graph/fields/label-field-field'
import type { AgentGraphPhaseForm } from '#components/strategy-settings/agent-graph/types'

const meta = {
  title: 'StrategySettings/AgentGraph/LabelFieldField',
  component: LabelFieldField,
} satisfies Meta<typeof LabelFieldField>

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
      items: {
        name: { type: 'string' },
        description: { type: 'string' },
      },
    },
  },
}

const ALPHA_WITH_PRIMITIVE_ARRAY: AgentGraphPhaseForm = {
  ...ALPHA,
  output: {
    entries: { type: 'array', items: { type: 'string' } },
  },
}

function renderField(args: Story['args']) {
  return (
    <div className="grid grid-cols-(--grid-cols-field-label) items-start gap-x-3 gap-y-2 text-xs">
      <LabelFieldField {...args} />
    </div>
  )
}

export const Default: Story = {
  name: 'shows the label field with available output properties.',
  args: {
    phases: [ALPHA],
    forEach: 'alpha.records',
    value: undefined,
    onChange: () => {},
  },
  render: renderField,
}

export const Selected: Story = {
  name: 'shows a selected label from an earlier phase output.',
  args: {
    phases: [ALPHA],
    forEach: 'alpha.records',
    value: 'name',
    onChange: () => {},
  },
  render: renderField,
}

export const StaleValue: Story = {
  name: 'preserves a selected label that is missing from the output properties.',
  args: {
    phases: [ALPHA],
    forEach: 'alpha.records',
    value: 'removed_field',
    onChange: () => {},
  },
  render: renderField,
}

export const NoOptions: Story = {
  name: 'shows the label field when array items have no named properties.',
  args: {
    phases: [ALPHA_WITH_PRIMITIVE_ARRAY],
    forEach: 'alpha.entries',
    value: undefined,
    onChange: () => {},
  },
  render: renderField,
}
