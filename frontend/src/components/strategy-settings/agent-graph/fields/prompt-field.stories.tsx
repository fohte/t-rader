import type { Meta, StoryObj } from '@storybook/react-vite'
import { useState } from 'react'

import { PromptField } from '#components/strategy-settings/agent-graph/fields/prompt-field'

const meta = {
  title: 'StrategySettings/AgentGraph/PromptField',
  component: PromptField,
} satisfies Meta<typeof PromptField>

export default meta
type Story = StoryObj<typeof meta>

// PhaseCard のフィールドグリッド (grid-cols-[108px_1fr]) を再現し、実際の見え方に合わせる
function Interactive({ initial }: { initial: string }) {
  const [value, setValue] = useState(initial)
  return (
    <div className="grid grid-cols-[108px_1fr] items-start gap-x-3 gap-y-2 text-[12px]">
      <PromptField value={value} onChange={setValue} />
    </div>
  )
}

const SAMPLE_PROMPT =
  '与えられた問いに対し、検証すべき仮説を 2-4 件立てよ。\n\n[[indicator:USDJPY]] の direction も考慮すること。'

export const Default: Story = {
  args: { value: SAMPLE_PROMPT, onChange: () => {} },
  render: () => <Interactive initial={SAMPLE_PROMPT} />,
}

export const Empty: Story = {
  args: { value: '', onChange: () => {} },
  render: () => <Interactive initial="" />,
}
