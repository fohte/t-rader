import type { Meta, StoryObj } from '@storybook/react-vite'
import { useState } from 'react'

import { ModelField } from '#components/strategy-settings/agent-graph/fields/model-field'
import { PromptField } from '#components/strategy-settings/agent-graph/fields/prompt-field'
import { PhaseCard } from '#components/strategy-settings/agent-graph/phase-card'
import type { AgentGraphPhaseForm } from '#components/strategy-settings/agent-graph/types'

const meta = {
  title: 'StrategySettings/AgentGraph/PhaseCard',
  component: PhaseCard,
} satisfies Meta<typeof PhaseCard>

export default meta
type Story = StoryObj<typeof meta>

const PLAN_PHASE: AgentGraphPhaseForm = {
  key: 'plan',
  label: '調査計画',
  model: 'claude-opus-4',
  prompt: '与えられた問いに対し、検証すべき仮説を 2-4 件立てよ。',
  skills: [],
  tools: [],
  output: {},
}

const INVESTIGATE_PHASE: AgentGraphPhaseForm = {
  key: 'investigate',
  label: '仮説の調査',
  model: 'deepseek-v4-flash',
  prompt: '割り当てられた 1 件だけを検証し、結論をノートに書け。',
  forEach: 'plan.hypotheses',
  labelField: 'title',
  skills: [],
  tools: [],
  output: {},
}

const UNSET_MODEL_PHASE: AgentGraphPhaseForm = {
  ...PLAN_PHASE,
  model: '',
}

function Interactive({
  initial,
  referencedLabel,
  hasError = false,
}: {
  initial: AgentGraphPhaseForm
  referencedLabel?: string
  hasError?: boolean
}) {
  const [phase, setPhase] = useState(initial)
  return (
    <PhaseCard
      index={0}
      total={2}
      phase={phase}
      referencedLabel={referencedLabel}
      hasError={hasError}
      onMoveUp={() => {}}
      onMoveDown={() => {}}
      onRemove={() => {}}
    >
      <ModelField
        value={phase.model}
        onChange={(model) => {
          setPhase({ ...phase, model })
        }}
      />
      <PromptField
        value={phase.prompt}
        onChange={(prompt) => {
          setPhase({ ...phase, prompt })
        }}
      />
    </PhaseCard>
  )
}

export const Default: Story = {
  args: {
    index: 0,
    total: 2,
    phase: PLAN_PHASE,
    onMoveUp: () => {},
    onMoveDown: () => {},
    onRemove: () => {},
    children: null,
  },
  render: () => <Interactive initial={PLAN_PHASE} />,
}

export const ForEach: Story = {
  args: {
    index: 1,
    total: 2,
    phase: INVESTIGATE_PHASE,
    referencedLabel: '調査計画',
    onMoveUp: () => {},
    onMoveDown: () => {},
    onRemove: () => {},
    children: null,
  },
  render: () => (
    <Interactive initial={INVESTIGATE_PHASE} referencedLabel="調査計画" />
  ),
}

export const WithError: Story = {
  args: {
    index: 0,
    total: 2,
    phase: PLAN_PHASE,
    hasError: true,
    onMoveUp: () => {},
    onMoveDown: () => {},
    onRemove: () => {},
    children: null,
  },
  render: () => <Interactive initial={PLAN_PHASE} hasError />,
}

export const ModelUnset: Story = {
  args: {
    index: 0,
    total: 2,
    phase: UNSET_MODEL_PHASE,
    onMoveUp: () => {},
    onMoveDown: () => {},
    onRemove: () => {},
    children: null,
  },
  render: () => <Interactive initial={UNSET_MODEL_PHASE} />,
}
