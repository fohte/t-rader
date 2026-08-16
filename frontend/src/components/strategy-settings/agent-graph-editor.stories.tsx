import type { Meta, StoryObj } from '@storybook/react-vite'

import { AgentGraphEditor } from '#components/strategy-settings/agent-graph-editor'

const meta = {
  title: 'StrategySettings/AgentGraphEditor',
  component: AgentGraphEditor,
} satisfies Meta<typeof AgentGraphEditor>

export default meta
type Story = StoryObj<typeof meta>

const SAMPLE = `phases:
  - key: plan
    label: 調査計画
    model: claude-opus-4
    runs: once
    prompt: 与えられた問いに対し、検証すべき仮説を 2-4 件立てよ。
    output:
      hypotheses:
        type: array
  - key: investigate
    label: 仮説の調査
    model: deepseek-v4-flash
    for_each: plan.hypotheses
    label_field: title
    max_parallel: 4
    prompt: 割り当てられた 1 件だけを検証し、結論をノートに書け。
  - key: synthesize
    label: 統合
    model: claude-sonnet-4
    prompt: 調査結果をまとめ、結論のノートを 1 本書け。
`

export const FormView: Story = {
  args: {
    initialValue: SAMPLE,
    onSave: () => {},
  },
}

export const PhaseSplitDisabled: Story = {
  args: {
    initialValue: '',
    onSave: () => {},
  },
}

export const BrokenYaml: Story = {
  args: {
    initialValue: 'phases: [',
    onSave: () => {},
  },
}

export const SaveErrorOnPhase: Story = {
  args: {
    initialValue: SAMPLE,
    onSave: () => {},
    saveError: 'phase key "plan" is duplicated',
  },
}

export const Saving: Story = {
  args: {
    initialValue: SAMPLE,
    onSave: () => {},
    isSaving: true,
  },
}
