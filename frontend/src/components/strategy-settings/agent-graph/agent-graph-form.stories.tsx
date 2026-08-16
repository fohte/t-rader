import type { Meta, StoryObj } from '@storybook/react-vite'
import { useState } from 'react'

import { AgentGraphForm } from '#components/strategy-settings/agent-graph/agent-graph-form'

const meta = {
  title: 'StrategySettings/AgentGraph/AgentGraphForm',
  component: AgentGraphForm,
} satisfies Meta<typeof AgentGraphForm>

export default meta
type Story = StoryObj<typeof meta>

const SINGLE_PHASE = `phases:
  - key: plan
    label: 調査計画
    model: claude-opus-4
    prompt: 与えられた問いに対し、検証すべき仮説を立てよ。
`

const MULTI_PHASE_WITH_FOR_EACH = `phases:
  - key: plan
    label: 調査計画
    model: claude-opus-4
    prompt: 与えられた問いに対し、検証すべき仮説を 2-4 件立てよ。
    output:
      hypotheses:
        type: array
  - key: investigate
    label: 仮説の調査
    model: deepseek-v4-flash
    for_each: plan.hypotheses
    label_field: title
    prompt: 割り当てられた 1 件だけを検証し、結論をノートに書け。
`

function Interactive({
  initial,
  errorPhaseKey = null,
}: {
  initial: string
  errorPhaseKey?: string | null
}) {
  const [value, setValue] = useState(initial)
  return (
    <AgentGraphForm
      value={value}
      onChange={setValue}
      errorPhaseKey={errorPhaseKey}
    />
  )
}

export const ToggleOff: Story = {
  args: { value: '', onChange: () => {} },
  render: () => <Interactive initial="" />,
}

export const ToggleOn: Story = {
  args: { value: SINGLE_PHASE, onChange: () => {} },
  render: () => <Interactive initial={SINGLE_PHASE} />,
}

export const MultiPhaseWithForEach: Story = {
  args: { value: MULTI_PHASE_WITH_FOR_EACH, onChange: () => {} },
  render: () => <Interactive initial={MULTI_PHASE_WITH_FOR_EACH} />,
}

export const WithError: Story = {
  args: {
    value: MULTI_PHASE_WITH_FOR_EACH,
    onChange: () => {},
    errorPhaseKey: 'investigate',
  },
  render: () => (
    <Interactive
      initial={MULTI_PHASE_WITH_FOR_EACH}
      errorPhaseKey="investigate"
    />
  ),
}
