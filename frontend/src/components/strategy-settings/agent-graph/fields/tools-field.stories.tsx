import type { Meta, StoryObj } from '@storybook/react-vite'

import { ToolsField } from '#components/strategy-settings/agent-graph/fields/tools-field'
import type { components } from '#lib/api/schema.gen'

type AgentTool = components['schemas']['AgentTool']

const TOOLS: AgentTool[] = [
  { name: 'tool_alpha', description: null },
  { name: 'tool_beta', description: null },
  { name: 'tool_gamma', description: null },
  { name: 'tool_delta', description: null },
]

const meta = {
  title: 'StrategySettings/AgentGraph/ToolsField',
  component: ToolsField,
} satisfies Meta<typeof ToolsField>

export default meta
type Story = StoryObj<typeof meta>

function renderField(args: Story['args']) {
  return (
    <div className="grid grid-cols-(--grid-cols-field-label) items-start gap-x-3 gap-y-2 text-xs">
      <ToolsField {...args} />
    </div>
  )
}

export const AllToolsAllowed: Story = {
  name: 'allows every available tool.',
  args: { value: undefined, onChange: () => {}, options: TOOLS },
  render: renderField,
}

export const Restricted: Story = {
  name: 'allows only a selected subset of tools.',
  args: {
    value: ['tool_alpha', 'tool_beta'],
    onChange: () => {},
    options: TOOLS,
  },
  render: renderField,
}

export const RestrictedToNone: Story = {
  name: 'allows no tools.',
  args: { value: [], onChange: () => {}, options: TOOLS },
  render: renderField,
}

export const ValueNotInOptions: Story = {
  name: 'preserves a selected tool that is missing from the available options.',
  args: { value: ['legacy_tool'], onChange: () => {}, options: TOOLS },
  render: renderField,
}
