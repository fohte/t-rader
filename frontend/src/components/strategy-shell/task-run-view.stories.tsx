import type { Meta, StoryObj } from '@storybook/react-vite'
import { RouterProvider } from '@tanstack/react-router'

import {
  CONFIG_PHASES,
  investigateStep,
  PLAN_STEP,
} from '#components/strategy-shell/task-execution-tree.fixtures'
import {
  TaskRunView,
  type TaskRunViewProps,
} from '#components/strategy-shell/task-run-view'
import { createStoryRouter } from '#storybook/story-router'

function createTaskRunViewRouter(props: TaskRunViewProps) {
  return createStoryRouter(
    () => (
      <div className="min-h-screen bg-[color:var(--color-bg-primary)] p-6">
        <TaskRunView {...props} />
      </div>
    ),
    { paths: ['/strategies/$id/runs', '/strategies/$id/notes/$noteId'] },
  )
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
      router={createTaskRunViewRouter({
        strategyId: 'semi-swing',
        task: baseTask({ phase: 'running' }),
        steps: [
          PLAN_STEP,
          investigateStep('円安の進行が主因', {
            status: 'completed',
            finishedAt: '2026-08-15T09:00:21.100Z',
            verdict: 'supported',
            noteId: 'note-0001',
          }),
          investigateStep('半導体サイクルの反転', { status: 'running' }),
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
      router={createTaskRunViewRouter({
        strategyId: 'semi-swing',
        task: baseTask({
          phase: 'completed',
          updatedAt: '2026-08-16T09:04:33.200Z',
        }),
        steps: [
          PLAN_STEP,
          investigateStep('円安の進行が主因', {
            status: 'completed',
            finishedAt: '2026-08-15T09:00:21.100Z',
            verdict: 'supported',
            noteId: 'note-0001',
          }),
          investigateStep('半導体サイクルの反転', {
            status: 'completed',
            finishedAt: '2026-08-15T09:00:24.900Z',
            verdict: 'rejected',
            noteId: 'note-0002',
          }),
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
      router={createTaskRunViewRouter({
        strategyId: 'semi-swing',
        task: baseTask({
          phase: 'failed',
          updatedAt: '2026-08-16T09:01:18.300Z',
          errorSummary: 'tool call timeout: query_data がタイムアウトしました',
        }),
        steps: [
          PLAN_STEP,
          {
            ...investigateStep('半導体サイクルの反転', { status: 'failed' }),
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
      router={createTaskRunViewRouter({
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
      router={createTaskRunViewRouter({
        strategyId: 'semi-swing',
        task: null,
        steps: [],
        configPhases: [],
        generatedNotesCount: 0,
      })}
    />
  ),
}
