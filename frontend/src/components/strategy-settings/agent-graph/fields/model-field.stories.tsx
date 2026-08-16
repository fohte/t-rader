import type { Meta, StoryObj } from '@storybook/react-vite'
import { useState } from 'react'

import { ModelField } from '#components/strategy-settings/agent-graph/fields/model-field'

const meta = {
  title: 'StrategySettings/AgentGraph/ModelField',
  component: ModelField,
} satisfies Meta<typeof ModelField>

export default meta
type Story = StoryObj<typeof meta>

// PhaseCard のフィールドグリッド (grid-cols-[108px_1fr]) を再現し、実際の見え方に合わせる
function Interactive({ initial }: { initial: string }) {
  const [value, setValue] = useState(initial)
  return (
    <div className="grid grid-cols-[108px_1fr] items-start gap-x-3 gap-y-2 text-[12px]">
      <ModelField value={value} onChange={setValue} />
    </div>
  )
}

export const Default: Story = {
  args: { value: 'claude-sonnet-5', onChange: () => {} },
  render: () => <Interactive initial="claude-sonnet-5" />,
}

export const Empty: Story = {
  args: { value: '', onChange: () => {} },
  render: () => <Interactive initial="" />,
}
