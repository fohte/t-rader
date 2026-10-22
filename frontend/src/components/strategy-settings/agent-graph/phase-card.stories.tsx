import type { Meta, StoryObj } from '@storybook/react-vite'

import { InputBindingsField } from '#components/strategy-settings/agent-graph/fields/input-bindings-field'
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

const ALPHA_PHASE: AgentGraphPhaseForm = {
  key: 'alpha',
  label: 'サンプル',
  model: 'fictional-model-orbit',
  prompt: 'sample prompt',
  skills: [],
  tools: [],
  output: {},
}

const BETA_PHASE: AgentGraphPhaseForm = {
  key: 'beta',
  label: '次の段階',
  model: 'fictional-model-lantern',
  prompt: 'sample prompt',
  forEach: 'alpha.records',
  labelField: 'name',
  skills: [],
  tools: [],
  output: {},
}

const UNSET_MODEL_PHASE: AgentGraphPhaseForm = {
  ...ALPHA_PHASE,
  model: '',
}

const OPERATION_PHASE: AgentGraphPhaseForm = {
  key: 'calculate',
  label: '計算',
  kind: 'operation',
  operation: 'evaluate_saved_code',
  codeId: 'sample-code',
  input: {
    request: { literal: { count: 2 } },
    source: {
      dataset: { provider: 'sample-provider', params: { code: 'DEMO' } },
    },
  },
  output: { result: { type: 'number' } },
}

function renderPhase(args: Story['args']) {
  const phase = args.phase
  return (
    <PhaseCard
      index={args.index}
      total={args.total}
      phase={phase}
      referencedLabel={args.referencedLabel}
      hasError={args.hasError}
      onLabelChange={args.onLabelChange}
      onKindChange={args.onKindChange}
      onMoveUp={args.onMoveUp}
      onMoveDown={args.onMoveDown}
      onRemove={args.onRemove}
    >
      {phase.kind === 'operation' ? (
        <>
          <span>保存コード ID</span>
          <span className="font-mono">{phase.codeId}</span>
          <InputBindingsField value={phase.input} onChange={() => {}} />
        </>
      ) : (
        <>
          <ModelField value={phase.model} onChange={() => {}} models={[]} />
          <PromptField value={phase.prompt} onChange={() => {}} />
        </>
      )}
    </PhaseCard>
  )
}

export const Default: Story = {
  name: 'shows a phase card with its model and prompt fields.',
  args: {
    index: 0,
    total: 2,
    phase: ALPHA_PHASE,
    onLabelChange: () => {},
    onKindChange: () => {},
    onMoveUp: () => {},
    onMoveDown: () => {},
    onRemove: () => {},
    children: null,
  },
  render: renderPhase,
}

export const ForEach: Story = {
  name: 'shows a phase card that runs for each item from an earlier phase.',
  args: {
    index: 1,
    total: 2,
    phase: BETA_PHASE,
    referencedLabel: 'サンプル',
    onLabelChange: () => {},
    onKindChange: () => {},
    onMoveUp: () => {},
    onMoveDown: () => {},
    onRemove: () => {},
    children: null,
  },
  render: renderPhase,
}

export const WithError: Story = {
  name: 'shows a phase card with a validation error.',
  args: {
    index: 0,
    total: 2,
    phase: ALPHA_PHASE,
    hasError: true,
    onLabelChange: () => {},
    onKindChange: () => {},
    onMoveUp: () => {},
    onMoveDown: () => {},
    onRemove: () => {},
    children: null,
  },
  render: renderPhase,
}

export const ModelUnset: Story = {
  name: 'shows a phase card without a selected model.',
  args: {
    index: 0,
    total: 2,
    phase: UNSET_MODEL_PHASE,
    onLabelChange: () => {},
    onKindChange: () => {},
    onMoveUp: () => {},
    onMoveDown: () => {},
    onRemove: () => {},
    children: null,
  },
  render: renderPhase,
}

export const Operation: Story = {
  name: 'shows a saved-code operation with its input bindings.',
  args: {
    index: 0,
    total: 1,
    phase: OPERATION_PHASE,
    onLabelChange: () => {},
    onKindChange: () => {},
    onMoveUp: () => {},
    onMoveDown: () => {},
    onRemove: () => {},
    children: null,
  },
  render: renderPhase,
}
