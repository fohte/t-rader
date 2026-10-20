import type { Meta, StoryObj } from '@storybook/react-vite'
import { useState } from 'react'

import { PromptField } from '#components/strategy-settings/agent-graph/fields/prompt-field'

const meta = {
  title: 'StrategySettings/AgentGraph/PromptField',
  component: PromptField,
} satisfies Meta<typeof PromptField>

export default meta
type Story = StoryObj<typeof meta>

// PhaseCard のフィールドグリッド (grid-cols-(--grid-cols-field-label)) を再現し、実際の見え方に合わせる
function Interactive({ initial }: { initial: string }) {
  const [value, setValue] = useState(initial)
  return (
    <div className="grid grid-cols-(--grid-cols-field-label) items-start gap-x-3 gap-y-2 text-xs">
      <PromptField value={value} onChange={setValue} />
    </div>
  )
}

const SAMPLE_PROMPT =
  'テーマを読みやすい見出しに分ける。\n\n[[group:sample-archive/collection-a]] の内容も参照すること。'

export const Default: Story = {
  name: 'shows a prompt containing an internal reference.',
  args: { value: SAMPLE_PROMPT, onChange: () => {} },
  render: () => <Interactive initial={SAMPLE_PROMPT} />,
}

export const Empty: Story = {
  name: 'shows an empty prompt field.',
  args: { value: '', onChange: () => {} },
  render: () => <Interactive initial="" />,
}
