import type { Meta, StoryObj } from '@storybook/react-vite'
import {
  createMemoryHistory,
  createRootRoute,
  createRoute,
  createRouter,
  RouterProvider,
} from '@tanstack/react-router'

import {
  type AgentGraphPhaseSummary,
  TaskExecutionTree,
  type TaskStep,
} from '#components/strategy-shell/task-execution-tree'

// ノートリンクが親ルートを要求するため、Frame 自体をルーターで包む
function Frame({ children }: { children: React.ReactNode }) {
  const rootRoute = createRootRoute({
    component: () => (
      <div className="min-h-screen bg-[color:var(--color-bg-primary)] p-6">
        <div className="w-[420px] border border-[color:var(--color-border-strategy)] bg-[color:var(--color-bg-secondary)] p-3.5">
          {children}
        </div>
      </div>
    ),
  })
  const noteRoute = createRoute({
    getParentRoute: () => rootRoute,
    path: '/strategies/$id/notes/$noteId',
    component: () => null,
  })
  const router = createRouter({
    routeTree: rootRoute.addChildren([noteRoute]),
    history: createMemoryHistory({ initialEntries: ['/'] }),
  })
  return <RouterProvider router={router} />
}

const CONFIG_PHASES: AgentGraphPhaseSummary[] = [
  { key: 'plan', label: '調査計画', model: 'claude-opus-4' },
  {
    key: 'investigate',
    label: '仮説の調査',
    model: 'deepseek-v4-flash',
    output: {
      verdict: { enum: ['supported', 'rejected', 'inconclusive'] },
      summary: { type: 'string' },
      note_id: { type: 'string' },
    },
  },
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
  verdict?: 'supported' | 'rejected' | 'inconclusive',
  noteId?: string,
): TaskStep {
  return {
    phase_key: 'investigate',
    label: '仮説の調査',
    model: 'deepseek-v4-flash',
    status,
    item: { title },
    item_label: title,
    output:
      status === 'completed'
        ? {
            verdict: verdict ?? 'supported',
            summary: `${title}を検証した結果`,
            ...(noteId != null ? { note_id: noteId } : {}),
          }
        : undefined,
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

// merge フェーズには step を渡さないので、常に「未着手」ノードとして描画される
export const Running: Story = {
  render: () => (
    <Frame>
      <TaskExecutionTree
        strategyId="semi-swing"
        steps={[
          PLAN_STEP,
          investigateStep(
            '円安の進行が主因',
            'completed',
            '2026-08-15T09:00:21.100Z',
            'supported',
            'note-0001',
          ),
          investigateStep('半導体サイクルの反転', 'running', undefined),
          investigateStep(
            '個別の材料出尽くし',
            'completed',
            '2026-08-15T09:00:19.700Z',
            'rejected',
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
        strategyId="semi-swing"
        steps={[
          PLAN_STEP,
          investigateStep(
            '円安の進行が主因',
            'completed',
            '2026-08-15T09:00:21.100Z',
            'supported',
            'note-0001',
          ),
          investigateStep(
            '半導体サイクルの反転',
            'completed',
            '2026-08-15T09:00:24.900Z',
            'rejected',
            'note-0002',
          ),
          investigateStep(
            '個別の材料出尽くし',
            'completed',
            '2026-08-15T09:00:19.700Z',
            'inconclusive',
          ),
          {
            phase_key: 'merge',
            label: '統合',
            model: 'claude-sonnet-4',
            status: 'completed',
            output: {
              summary: '円安進行と半導体サイクル反転の複合要因と判断',
              note_id: 'note-0003',
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
        strategyId="semi-swing"
        steps={[
          PLAN_STEP,
          investigateStep(
            '円安の進行が主因',
            'completed',
            '2026-08-15T09:00:21.100Z',
            'supported',
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
            'rejected',
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
      <TaskExecutionTree strategyId="semi-swing" steps={[]} configPhases={[]} />
    </Frame>
  ),
}
