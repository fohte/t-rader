import type { Meta, StoryObj } from '@storybook/react-vite'

import {
  type AgentGraphPhaseSummary,
  TaskExecutionTree,
  type TaskStep,
} from '#components/strategy-shell/task-execution-tree'

function Frame({ children }: { children: React.ReactNode }) {
  return (
    <div className="min-h-screen bg-[color:var(--color-bg-primary)] p-6">
      <div className="w-[420px] border border-[color:var(--color-border-strategy)] bg-[color:var(--color-bg-secondary)] p-3.5">
        {children}
      </div>
    </div>
  )
}

const CONFIG_PHASES: AgentGraphPhaseSummary[] = [
  { key: 'plan', label: '調査計画', model: 'claude-opus-4' },
  { key: 'investigate', label: '仮説の調査', model: 'deepseek-v4-flash' },
  { key: 'merge', label: '統合', model: 'claude-sonnet-4' },
]

const PLAN_STEP: TaskStep = {
  phase_key: 'plan',
  label: '調査計画',
  model: 'claude-opus-4',
  status: 'completed',
  output: {
    items: ['円安の進行が主因', '半導体サイクルの反転', '個別の材料出尽くし'],
  },
  started_at: '2026-08-15T09:00:00.000Z',
  finished_at: '2026-08-15T09:00:12.400Z',
  trace_id: 'trace-plan-0001',
  span_id: 'span-plan-0001',
}

function investigateStep(
  title: string,
  status: TaskStep['status'],
  finishedAt: string | undefined,
): TaskStep {
  return {
    phase_key: 'investigate',
    label: '仮説の調査',
    model: 'deepseek-v4-flash',
    status,
    item: { title },
    item_label: title,
    output:
      status === 'completed' ? { verdict: '妥当', confidence: 0.7 } : undefined,
    started_at: '2026-08-15T09:00:13.000Z',
    finished_at: finishedAt,
    trace_id: `trace-investigate-${title}`,
    span_id: `span-investigate-${title}`,
  }
}

const meta = {
  title: 'StrategyShell/TaskExecutionTree',
  parameters: { layout: 'fullscreen' },
} satisfies Meta

export default meta
type Story = StoryObj<typeof meta>

export const Running: Story = {
  render: () => (
    <Frame>
      <TaskExecutionTree
        steps={[
          PLAN_STEP,
          investigateStep(
            '円安の進行が主因',
            'completed',
            '2026-08-15T09:00:21.100Z',
          ),
          investigateStep('半導体サイクルの反転', 'running', undefined),
          investigateStep(
            '個別の材料出尽くし',
            'completed',
            '2026-08-15T09:00:19.700Z',
          ),
        ]}
        configPhases={CONFIG_PHASES}
        traceUrlTemplate="https://grafana.example/explore?traceID={trace_id}&spanID={span_id}"
      />
    </Frame>
  ),
}

export const Completed: Story = {
  render: () => (
    <Frame>
      <TaskExecutionTree
        steps={[
          PLAN_STEP,
          investigateStep(
            '円安の進行が主因',
            'completed',
            '2026-08-15T09:00:21.100Z',
          ),
          investigateStep(
            '半導体サイクルの反転',
            'completed',
            '2026-08-15T09:00:24.900Z',
          ),
          investigateStep(
            '個別の材料出尽くし',
            'completed',
            '2026-08-15T09:00:19.700Z',
          ),
          {
            phase_key: 'merge',
            label: '統合',
            model: 'claude-sonnet-4',
            status: 'completed',
            output: {
              summary: '円安進行と半導体サイクル反転の複合要因と判断',
            },
            started_at: '2026-08-15T09:00:25.000Z',
            finished_at: '2026-08-15T09:00:33.200Z',
            trace_id: 'trace-merge-0001',
            span_id: 'span-merge-0001',
          },
        ]}
        configPhases={CONFIG_PHASES}
        traceUrlTemplate="https://grafana.example/explore?traceID={trace_id}&spanID={span_id}"
      />
    </Frame>
  ),
}

export const WithFailure: Story = {
  render: () => (
    <Frame>
      <TaskExecutionTree
        steps={[
          PLAN_STEP,
          investigateStep(
            '円安の進行が主因',
            'completed',
            '2026-08-15T09:00:21.100Z',
          ),
          {
            ...investigateStep('半導体サイクルの反転', 'failed', undefined),
            finished_at: '2026-08-15T09:00:18.300Z',
            error: 'tool call timeout: query_data がタイムアウトしました',
            output: undefined,
          },
          investigateStep(
            '個別の材料出尽くし',
            'completed',
            '2026-08-15T09:00:19.700Z',
          ),
        ]}
        configPhases={CONFIG_PHASES}
      />
    </Frame>
  ),
}

export const NoAgentGraph: Story = {
  render: () => (
    <Frame>
      <p className="font-mono text-[11px] text-[color:var(--color-text-tertiary)]">
        agent_graph 未設定 (steps が空) — 以下、何も表示されません:
      </p>
      <TaskExecutionTree steps={[]} configPhases={[]} />
    </Frame>
  ),
}
