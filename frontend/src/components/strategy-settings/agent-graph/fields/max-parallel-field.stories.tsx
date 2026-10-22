import type { Meta, StoryObj } from '@storybook/react-vite'

import { MaxParallelField } from '#components/strategy-settings/agent-graph/fields/max-parallel-field'

const meta = {
  title: 'StrategySettings/AgentGraph/MaxParallelField',
  component: MaxParallelField,
} satisfies Meta<typeof MaxParallelField>

export default meta
type Story = StoryObj<typeof meta>

function renderField(args: Story['args']) {
  return (
    <div className="grid grid-cols-(--grid-cols-field-label) items-start gap-x-3 gap-y-2 text-xs">
      <MaxParallelField {...args} />
    </div>
  )
}

export const Default: Story = {
  name: 'sets a limit of four parallel runs.',
  args: { value: 4, onChange: () => {} },
  render: renderField,
}

export const Unset: Story = {
  name: 'shows the parallel run limit before a value is set.',
  args: { value: undefined, onChange: () => {} },
  render: renderField,
}
