import type { Meta, StoryObj } from '@storybook/react-vite'
import { useState } from 'react'

import { SkillsField } from '#components/strategy-settings/agent-graph/fields/skills-field'

const OPTIONS = ['snapshot', 'recap']

const meta = {
  title: 'StrategySettings/AgentGraph/SkillsField',
  component: SkillsField,
} satisfies Meta<typeof SkillsField>

export default meta
type Story = StoryObj<typeof meta>

function Interactive({ initial }: { initial: string[] }) {
  const [value, setValue] = useState(initial)
  return (
    <div className="grid grid-cols-[108px_1fr] items-start gap-x-3 gap-y-2 text-xs">
      <SkillsField value={value} onChange={setValue} options={OPTIONS} />
    </div>
  )
}

export const Empty: Story = {
  args: { value: [], onChange: () => {}, options: OPTIONS },
  render: () => <Interactive initial={[]} />,
}

export const Selected: Story = {
  args: { value: ['snapshot'], onChange: () => {}, options: OPTIONS },
  render: () => <Interactive initial={['snapshot']} />,
}
