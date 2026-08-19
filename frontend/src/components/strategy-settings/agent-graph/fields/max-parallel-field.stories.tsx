import type { Meta, StoryObj } from '@storybook/react-vite'
import { useState } from 'react'

import { MaxParallelField } from '#components/strategy-settings/agent-graph/fields/max-parallel-field'

const meta = {
  title: 'StrategySettings/AgentGraph/MaxParallelField',
  component: MaxParallelField,
} satisfies Meta<typeof MaxParallelField>

export default meta
type Story = StoryObj<typeof meta>

// PhaseCard のフィールドグリッド (grid-cols-[108px_1fr]) を再現し、実際の見え方に合わせる
function Interactive({ initial }: { initial: number | undefined }) {
  const [value, setValue] = useState(initial)
  return (
    <div className="grid grid-cols-[108px_1fr] items-start gap-x-3 gap-y-2 text-[12px]">
      <MaxParallelField value={value} onChange={setValue} />
    </div>
  )
}

export const Default: Story = {
  args: { value: 4, onChange: () => {} },
  render: () => <Interactive initial={4} />,
}

export const Unset: Story = {
  args: { value: undefined, onChange: () => {} },
  render: () => <Interactive initial={undefined} />,
}
