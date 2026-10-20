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
  label: '見出し案',
  model: 'fictional-model-orbit',
  prompt: '与えられたテーマを 2-4 個の見出しに分ける。',
  skills: [],
  tools: [],
  output: {},
}

const COMPOSE_PHASE: AgentGraphPhaseForm = {
  key: 'compose',
  label: '節の作成',
  model: 'fictional-model-lantern',
  prompt: '割り当てられた見出しに沿って、説明文を作成する。',
  forEach: 'plan.sections',
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
      onLabelChange={(label) => {
        setPhase({ ...phase, label })
      }}
      onMoveUp={() => {}}
      onMoveDown={() => {}}
      onRemove={() => {}}
    >
      <ModelField
        value={phase.model}
        onChange={(model) => {
          setPhase({ ...phase, model })
        }}
        models={[]}
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
  name: 'shows a phase card with its model and prompt fields.',
  args: {
    index: 0,
    total: 2,
    phase: PLAN_PHASE,
    onLabelChange: () => {},
    onMoveUp: () => {},
    onMoveDown: () => {},
    onRemove: () => {},
    children: null,
  },
  render: () => <Interactive initial={PLAN_PHASE} />,
}

export const ForEach: Story = {
  name: 'shows a phase card that runs for each item from an earlier phase.',
  args: {
    index: 1,
    total: 2,
    phase: COMPOSE_PHASE,
    referencedLabel: '見出し案',
    onLabelChange: () => {},
    onMoveUp: () => {},
    onMoveDown: () => {},
    onRemove: () => {},
    children: null,
  },
  render: () => (
    <Interactive initial={COMPOSE_PHASE} referencedLabel="見出し案" />
  ),
}

export const WithError: Story = {
  name: 'shows a phase card with a validation error.',
  args: {
    index: 0,
    total: 2,
    phase: PLAN_PHASE,
    hasError: true,
    onLabelChange: () => {},
    onMoveUp: () => {},
    onMoveDown: () => {},
    onRemove: () => {},
    children: null,
  },
  render: () => <Interactive initial={PLAN_PHASE} hasError />,
}

export const ModelUnset: Story = {
  name: 'shows a phase card without a selected model.',
  args: {
    index: 0,
    total: 2,
    phase: UNSET_MODEL_PHASE,
    onLabelChange: () => {},
    onMoveUp: () => {},
    onMoveDown: () => {},
    onRemove: () => {},
    children: null,
  },
  render: () => <Interactive initial={UNSET_MODEL_PHASE} />,
}
