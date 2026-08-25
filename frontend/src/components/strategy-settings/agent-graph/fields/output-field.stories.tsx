import type { Meta, StoryObj } from '@storybook/react-vite'
import { useState } from 'react'

import { OutputField } from '#components/strategy-settings/agent-graph/fields/output-field'

const meta = {
  title: 'StrategySettings/AgentGraph/OutputField',
  component: OutputField,
} satisfies Meta<typeof OutputField>

export default meta
type Story = StoryObj<typeof meta>

// PhaseCard のフィールドグリッド (grid-cols-[108px_1fr]) を再現し、実際の見え方に合わせる
function Interactive({ initial }: { initial: Record<string, unknown> }) {
  const [value, setValue] = useState(initial)
  return (
    <div className="grid grid-cols-[108px_1fr] items-start gap-x-3 gap-y-2 text-xs">
      <OutputField value={value} onChange={setValue} />
    </div>
  )
}

const VALID_OUTPUT = {
  hypotheses: {
    type: 'array',
    description: '検証すべき仮説。2-4 件',
    items: {
      title: {
        type: 'string',
        description: '仮説を 1 文で言い切ったもの',
      },
      rationale: {
        type: 'string',
        description: 'なぜその仮説が立つか',
      },
    },
    required: ['title', 'rationale'],
  },
}

const INVALID_OUTPUT = {
  verdict: {
    enum: 'supported',
    description: 'checks を当てた結果',
  },
}

export const Valid: Story = {
  args: { value: VALID_OUTPUT, onChange: () => {} },
  render: () => <Interactive initial={VALID_OUTPUT} />,
}

export const Invalid: Story = {
  args: { value: INVALID_OUTPUT, onChange: () => {} },
  render: () => <Interactive initial={INVALID_OUTPUT} />,
}

export const Empty: Story = {
  args: { value: {}, onChange: () => {} },
  render: () => <Interactive initial={{}} />,
}
