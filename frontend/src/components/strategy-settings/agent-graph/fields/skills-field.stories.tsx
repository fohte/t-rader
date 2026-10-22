import type { Meta, StoryObj } from '@storybook/react-vite'

import { SkillsField } from '#components/strategy-settings/agent-graph/fields/skills-field'

const OPTIONS = ['skill_alpha', 'skill_beta']

const meta = {
  title: 'StrategySettings/AgentGraph/SkillsField',
  component: SkillsField,
} satisfies Meta<typeof SkillsField>

export default meta
type Story = StoryObj<typeof meta>

function renderField(args: Story['args']) {
  return (
    <div className="grid grid-cols-(--grid-cols-field-label) items-start gap-x-3 gap-y-2 text-xs">
      <SkillsField {...args} />
    </div>
  )
}

export const Empty: Story = {
  name: 'shows the skills field with no selection.',
  args: { value: [], onChange: () => {}, options: OPTIONS },
  render: renderField,
}

export const Selected: Story = {
  name: 'shows the skills field with one selected skill.',
  args: { value: ['skill_alpha'], onChange: () => {}, options: OPTIONS },
  render: renderField,
}
