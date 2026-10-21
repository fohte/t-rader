import type { Meta, StoryObj } from '@storybook/react-vite'

import { PromptField } from '#components/strategy-settings/agent-graph/fields/prompt-field'

const meta = {
  title: 'StrategySettings/AgentGraph/PromptField',
  component: PromptField,
} satisfies Meta<typeof PromptField>

export default meta
type Story = StoryObj<typeof meta>

const SAMPLE_PROMPT = 'sample prompt\n\n[[group:sample-axis/group-a]]'

function renderField(args: Story['args']) {
  return (
    <div className="grid grid-cols-(--grid-cols-field-label) items-start gap-x-3 gap-y-2 text-xs">
      <PromptField {...args} />
    </div>
  )
}

export const Default: Story = {
  name: 'shows a prompt with a prefixed internal reference.',
  args: { value: SAMPLE_PROMPT, onChange: () => {} },
  render: renderField,
}

export const Empty: Story = {
  name: 'shows an empty prompt field.',
  args: { value: '', onChange: () => {} },
  render: renderField,
}
