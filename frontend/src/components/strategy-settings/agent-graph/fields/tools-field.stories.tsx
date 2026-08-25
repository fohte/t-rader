import type { Meta, StoryObj } from '@storybook/react-vite'
import { useState } from 'react'

import { ToolsField } from '#components/strategy-settings/agent-graph/fields/tools-field'
import type { components } from '#lib/api/schema.gen'

type AgentTool = components['schemas']['AgentTool']

// backend/src/handlers/agent_options.rs に実在する登録済み tool 名
const TOOLS: AgentTool[] = [
  { name: 'list_notes', description: null },
  { name: 'query_data', description: null },
  { name: 'read_annotations', description: null },
  { name: 'write_note', description: null },
]

const meta = {
  title: 'StrategySettings/AgentGraph/ToolsField',
  component: ToolsField,
} satisfies Meta<typeof ToolsField>

export default meta
type Story = StoryObj<typeof meta>

function Interactive({ initial }: { initial: string[] | undefined }) {
  const [value, setValue] = useState(initial)
  return (
    <div className="grid grid-cols-(--grid-cols-field-label) items-start gap-x-3 gap-y-2 text-xs">
      <ToolsField value={value} onChange={setValue} options={TOOLS} />
    </div>
  )
}

export const AllToolsAllowed: Story = {
  args: { value: undefined, onChange: () => {}, options: TOOLS },
  render: () => <Interactive initial={undefined} />,
}

export const Restricted: Story = {
  args: {
    value: ['list_notes', 'query_data'],
    onChange: () => {},
    options: TOOLS,
  },
  render: () => <Interactive initial={['list_notes', 'query_data']} />,
}

export const RestrictedToNone: Story = {
  args: { value: [], onChange: () => {}, options: TOOLS },
  render: () => <Interactive initial={[]} />,
}

// options に無い値 (未登録・削除済みなど) も消えずに表示される
export const ValueNotInOptions: Story = {
  args: { value: ['legacy_tool'], onChange: () => {}, options: TOOLS },
  render: () => <Interactive initial={['legacy_tool']} />,
}
