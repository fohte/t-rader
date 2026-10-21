import type { Meta, StoryObj } from '@storybook/react-vite'

import { OutputField } from '#components/strategy-settings/agent-graph/fields/output-field'

const meta = {
  title: 'StrategySettings/AgentGraph/OutputField',
  component: OutputField,
} satisfies Meta<typeof OutputField>

export default meta
type Story = StoryObj<typeof meta>

function renderField(args: Story['args']) {
  return (
    <div className="grid grid-cols-(--grid-cols-field-label) items-start gap-x-3 gap-y-2 text-xs">
      <OutputField {...args} />
    </div>
  )
}

const VALID_OUTPUT = {
  sample_field: {
    type: 'array',
    description: 'sample array',
    items: {
      name: {
        type: 'string',
        description: 'sample name',
      },
      value: {
        type: 'string',
        description: 'sample value',
      },
    },
    required: ['name', 'value'],
  },
}

const INVALID_OUTPUT = {
  sample_field: {
    enum: 'sample-value',
    description: 'sample result',
  },
}

export const Valid: Story = {
  name: 'shows a valid structured output schema.',
  args: { value: VALID_OUTPUT, onChange: () => {} },
  render: renderField,
}

export const Invalid: Story = {
  name: 'shows a schema with an invalid field definition.',
  args: { value: INVALID_OUTPUT, onChange: () => {} },
  render: renderField,
}

export const Empty: Story = {
  name: 'shows an empty output schema.',
  args: { value: {}, onChange: () => {} },
  render: renderField,
}
