import type { Meta, StoryObj } from '@storybook/react-vite'
import {
  createMemoryHistory,
  createRootRoute,
  createRoute,
  createRouter,
  RouterProvider,
} from '@tanstack/react-router'

import type {
  AgentGraphPhaseSummary,
  TaskStep,
} from '#components/strategy-shell/task-execution-tree'
import {
  TaskRunView,
  type TaskRunViewProps,
} from '#components/strategy-shell/task-run-view'

function createStoryRouter(props: TaskRunViewProps) {
  const rootRoute = createRootRoute({
    component: () => (
      <div className="min-h-screen bg-[color:var(--color-bg-primary)] p-6">
        <TaskRunView {...props} />
      </div>
    ),
  })
  const runsListRoute = createRoute({
    getParentRoute: () => rootRoute,
    path: '/strategies/$id/runs',
    component: () => null,
  })
  return createRouter({
    routeTree: rootRoute.addChildren([runsListRoute]),
    history: createMemoryHistory({ initialEntries: ['/'] }),
  })
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

function baseTask(
  overrides: Partial<NonNullable<TaskRunViewProps['task']>> = {},
): NonNullable<TaskRunViewProps['task']> {
  return {
    taskId: 'task-0001',
    prompt: '半導体セクターの調整は一時的か、循環の転換点か',
    source: 'frontend',
    phase: 'running',
    createdAt: '2026-08-16T09:00:00.000Z',
    updatedAt: '2026-08-16T09:00:00.000Z',
    errorSummary: null,
    ...overrides,
  }
}

const meta = {
  title: 'StrategyShell/TaskRunView',
  parameters: { layout: 'fullscreen' },
} satisfies Meta

export default meta
type Story = StoryObj<typeof meta>

export const Running: Story = {
  render: () => (
    <RouterProvider
      router={createStoryRouter({
        strategyId: 'semi-swing',
        task: baseTask({ phase: 'running' }),
        steps: [
          PLAN_STEP,
          investigateStep(
            '円安の進行が主因',
            'completed',
            '2026-08-15T09:00:21.100Z',
          ),
          investigateStep('半導体サイクルの反転', 'running', undefined),
        ],
        configPhases: CONFIG_PHASES,
        generatedNotesCount: 0,
        traceUrlTemplate:
          'https://grafana.example/explore?traceID={trace_id}&spanID={span_id}',
      })}
    />
  ),
}

export const Completed: Story = {
  render: () => (
    <RouterProvider
      router={createStoryRouter({
        strategyId: 'semi-swing',
        task: baseTask({
          phase: 'completed',
          updatedAt: '2026-08-16T09:04:33.200Z',
        }),
        steps: [
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
        ],
        configPhases: CONFIG_PHASES,
        generatedNotesCount: 2,
      })}
    />
  ),
}

export const Failed: Story = {
  render: () => (
    <RouterProvider
      router={createStoryRouter({
        strategyId: 'semi-swing',
        task: baseTask({
          phase: 'failed',
          updatedAt: '2026-08-16T09:01:18.300Z',
          errorSummary: 'tool call timeout: query_data がタイムアウトしました',
        }),
        steps: [
          PLAN_STEP,
          {
            ...investigateStep('半導体サイクルの反転', 'failed', undefined),
            finished_at: '2026-08-16T09:01:18.300Z',
            error: 'tool call timeout: query_data がタイムアウトしました',
            output: undefined,
          },
        ],
        configPhases: CONFIG_PHASES,
        generatedNotesCount: 0,
      })}
    />
  ),
}

export const NoSteps: Story = {
  render: () => (
    <RouterProvider
      router={createStoryRouter({
        strategyId: 'semi-swing',
        task: baseTask({ phase: 'pending' }),
        steps: [],
        configPhases: [],
        generatedNotesCount: 0,
      })}
    />
  ),
}

export const Loading: Story = {
  render: () => (
    <RouterProvider
      router={createStoryRouter({
        strategyId: 'semi-swing',
        task: null,
        steps: [],
        configPhases: [],
        generatedNotesCount: 0,
      })}
    />
  ),
}
