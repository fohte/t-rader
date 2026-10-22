import type { Meta, StoryObj } from '@storybook/react-vite'

import { ModelField } from '#components/strategy-settings/agent-graph/fields/model-field'
import type { components } from '#lib/api/schema.gen'

type AgentModel = components['schemas']['AgentModel']

const MODELS: AgentModel[] = [
  {
    id: 'fictional-model-orbit',
    providers: ['fictional-lab'],
    max_input_tokens: null,
    max_output_tokens: null,
    supports_reasoning: true,
  },
  {
    id: 'fictional-model-maple',
    providers: ['fictional-lab'],
    max_input_tokens: null,
    max_output_tokens: null,
    supports_reasoning: false,
  },
  {
    id: 'fictional-model-lantern',
    providers: ['fictional-lab'],
    max_input_tokens: null,
    max_output_tokens: null,
    supports_reasoning: false,
  },
]

const meta = {
  title: 'StrategySettings/AgentGraph/ModelField',
  component: ModelField,
} satisfies Meta<typeof ModelField>

export default meta
type Story = StoryObj<typeof meta>

function renderField(args: Story['args']) {
  return (
    <div className="grid grid-cols-(--grid-cols-field-label) items-start gap-x-3 gap-y-2 text-xs">
      <ModelField {...args} />
    </div>
  )
}

export const Default: Story = {
  name: 'shows a selected model from the available options.',
  args: { value: 'fictional-model-maple', onChange: () => {}, models: MODELS },
  render: renderField,
}

export const Unset: Story = {
  name: 'shows the model field without a selection.',
  args: { value: '', onChange: () => {}, models: MODELS },
  render: renderField,
}

export const NoModelsAvailable: Story = {
  name: 'shows the model field when no choices are available.',
  args: { value: 'fictional-model-maple', onChange: () => {}, models: [] },
  render: renderField,
}

export const ValueNotInList: Story = {
  name: 'preserves a selected model that is missing from the available options.',
  args: { value: 'retired-example-model', onChange: () => {}, models: MODELS },
  render: renderField,
}

export const ChoicesOpen: Story = {
  name: 'shows the available model choices in the open popup.',
  args: {
    value: 'fictional-model-maple',
    onChange: () => {},
    models: MODELS,
    defaultOpen: true,
  },
  render: renderField,
}
