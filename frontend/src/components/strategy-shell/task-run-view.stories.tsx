import type { Meta, StoryObj } from '@storybook/react-vite'
import { RouterProvider } from '@tanstack/react-router'

import {
  CONFIG_PHASES,
  inspectStep,
  PREPARATION_STEP,
} from '#components/strategy-shell/task-execution-tree.fixtures'
import {
  TaskRunView,
  type TaskRunViewProps,
} from '#components/strategy-shell/task-run-view'
import { createStoryRouter } from '#storybook/story-router'

function createTaskRunViewRouter(props: TaskRunViewProps) {
  return createStoryRouter(
    () => (
      <div className="min-h-screen bg-background p-6">
        <TaskRunView {...props} />
      </div>
    ),
    { paths: ['/strategies/$id/runs', '/notes/$noteId'] },
  )
}

function baseTask(
  overrides: Partial<NonNullable<TaskRunViewProps['task']>> = {},
): NonNullable<TaskRunViewProps['task']> {
  return {
    taskId: 'task-0001',
    prompt: 'サンプル項目の記録内容を確認する',
    source: 'frontend',
    phase: 'running',
    createdAt: '2026-08-16T09:00:00.000Z',
    updatedAt: '2026-08-16T09:00:00.000Z',
    errorSummary: null,
    ...overrides,
  }
}

function artifactStateView(overrides: Partial<TaskRunViewProps>) {
  return (
    <RouterProvider
      router={createTaskRunViewRouter({
        strategyId: 'sample-strategy',
        task: baseTask(),
        steps: [PREPARATION_STEP],
        configPhases: CONFIG_PHASES,
        notes: [],
        notesState: 'success',
        executionDefinition: {
          status: 'ready',
          digest: 'synthetic-digest',
          createdAt: '2026-08-16T09:00:00.000Z',
          definition: { agent_config: { agent_graph: 'phases: []' } },
        },
        ...overrides,
      })}
    />
  )
}

const meta = {
  title: 'StrategyShell/TaskRunView',
  parameters: { layout: 'fullscreen' },
} satisfies Meta

export default meta
type Story = StoryObj<typeof meta>

export const Running: Story = {
  name: 'shows an active task with its item-check progress.',
  render: () => (
    <RouterProvider
      router={createTaskRunViewRouter({
        strategyId: 'sample-strategy',
        task: baseTask({ phase: 'running' }),
        steps: [
          PREPARATION_STEP,
          inspectStep('見出しの表記ゆれ', {
            status: 'completed',
            finishedAt: '2026-08-15T09:00:21.100Z',
            result: 'consistent',
            noteId: 'note-0001',
          }),
          inspectStep('注記の抜け', { status: 'running' }),
        ],
        configPhases: CONFIG_PHASES,
        notes: [],
        notesState: 'loading',
        traceUrlTemplate:
          'https://grafana.example/explore?traceID={trace_id}&spanID={span_id}',
      })}
    />
  ),
}

export const Completed: Story = {
  name: 'shows a completed task with links to its notes.',
  render: () => (
    <RouterProvider
      router={createTaskRunViewRouter({
        strategyId: 'sample-strategy',
        task: baseTask({
          phase: 'completed',
          updatedAt: '2026-08-16T09:04:33.200Z',
        }),
        steps: [
          PREPARATION_STEP,
          inspectStep('見出しの表記ゆれ', {
            status: 'completed',
            finishedAt: '2026-08-15T09:00:21.100Z',
            result: 'consistent',
            noteId: 'note-0001',
          }),
          inspectStep('注記の抜け', {
            status: 'completed',
            finishedAt: '2026-08-15T09:00:24.900Z',
            result: 'needs_review',
            noteId: 'note-0002',
          }),
        ],
        configPhases: CONFIG_PHASES,
        notes: [
          { id: 'fake-note-001', title: '表記確認メモ' },
          { id: 'fake-note-002', title: '重複項目の補足' },
        ],
        notesState: 'success',
      })}
    />
  ),
}

export const OperationWithPayload: Story = {
  name: 'shows the fixed operation definition and a finite payload page.',
  render: () => (
    <RouterProvider
      router={createTaskRunViewRouter({
        strategyId: 'sample-strategy',
        task: baseTask({
          phase: 'completed',
          updatedAt: '2026-08-16T09:04:33.200Z',
        }),
        steps: [
          {
            phase_key: 'calculate',
            execution_step_id: 'sample-step-operation',
            kind: 'operation',
            label: '計算',
            model: null,
            status: 'completed',
            input_ref: {
              payload_inputs: [
                {
                  field: 'source',
                  payload_id: '00000000-0000-4000-8000-000000000001',
                  pointer: '/records',
                  cursor: 0,
                  limit: 25,
                },
              ],
            },
            output_ref: { id: '00000000-0000-4000-8000-000000000002' },
            output: {
              payloadRef: { id: '00000000-0000-4000-8000-000000000002' },
              pointer: '/value',
            },
            started_at: '2026-08-16T09:00:00.000Z',
            finished_at: '2026-08-16T09:00:08.000Z',
            trace_id: 'sample-trace-operation',
            span_id: 'sample-span-operation',
          },
        ],
        configPhases: [
          {
            key: 'calculate',
            label: '計算',
            kind: 'operation',
            operation: 'evaluate_saved_code',
            codeId: 'sample-code',
            model: null,
            definition: {
              key: 'calculate',
              label: '計算',
              kind: 'operation',
              operation: 'evaluate_saved_code',
              code_id: 'sample-code',
            },
          },
        ],
        notes: [],
        notesState: 'success',
        executionDefinition: {
          status: 'ready',
          digest: 'synthetic-digest',
          createdAt: '2026-08-16T09:00:00.000Z',
          definition: {
            agent_config: {
              agent_graph: 'phases:\n  - key: calculate\n    kind: operation',
            },
          },
        },
        executionDefinitionOpen: true,
        selectedPayload: {
          payloadId: '00000000-0000-4000-8000-000000000002',
          label: '出力',
          pointer: '/value',
        },
        payloadRead: {
          status: 'ready',
          manifest: {
            status: 'sealed',
            itemCount: 1,
            incompleteReason: null,
          },
          page: {
            data: { value: 'sample-result' },
            totalCount: 1,
            nextCursor: 1,
          },
        },
      })}
    />
  ),
}

export const IncompletePayload: Story = {
  name: 'shows the reason when a payload capture is incomplete.',
  render: () => (
    <RouterProvider
      router={createTaskRunViewRouter({
        strategyId: 'sample-strategy',
        task: baseTask({
          phase: 'failed',
          updatedAt: '2026-08-16T09:04:33.200Z',
        }),
        steps: [],
        configPhases: [],
        notes: [],
        notesState: 'success',
        selectedPayload: {
          payloadId: '00000000-0000-4000-8000-000000000003',
          label: '入力',
          pointer: '/records',
        },
        payloadRead: {
          status: 'ready',
          manifest: {
            status: 'incomplete',
            itemCount: 0,
            incompleteReason: 'source_revision_changed',
          },
          page: null,
        },
      })}
    />
  ),
}

export const DefinitionLoading: Story = {
  name: 'shows the loading state while the fixed definition is fetched.',
  render: () =>
    artifactStateView({ executionDefinition: { status: 'loading' } }),
}

export const DefinitionError: Story = {
  name: 'shows an error when the fixed definition cannot be fetched.',
  render: () => artifactStateView({ executionDefinition: { status: 'error' } }),
}

export const DefinitionMissing: Story = {
  name: 'shows that a legacy run has no fixed definition snapshot.',
  render: () =>
    artifactStateView({
      task: baseTask({ phase: 'completed' }),
      configPhases: [],
      executionDefinition: { status: 'missing' },
    }),
}

export const PayloadLoading: Story = {
  name: 'shows the loading state while a payload page is fetched.',
  render: () =>
    artifactStateView({
      selectedPayload: {
        payloadId: '00000000-0000-4000-8000-000000000004',
        label: '入力',
        pointer: '/records',
      },
      payloadRead: { status: 'loading' },
    }),
}

export const PayloadError: Story = {
  name: 'shows an error when a payload page cannot be fetched.',
  render: () =>
    artifactStateView({
      selectedPayload: {
        payloadId: '00000000-0000-4000-8000-000000000004',
        label: '入力',
        pointer: '/records',
      },
      payloadRead: { status: 'error' },
    }),
}

export const PayloadStaging: Story = {
  name: 'shows that a staging payload is not ready to display.',
  render: () =>
    artifactStateView({
      selectedPayload: {
        payloadId: '00000000-0000-4000-8000-000000000004',
        label: '入力',
        pointer: '/records',
      },
      payloadRead: {
        status: 'ready',
        manifest: {
          status: 'staging',
          itemCount: 0,
          incompleteReason: 'capture_in_progress',
        },
        page: null,
      },
    }),
}

export const PayloadPageMissing: Story = {
  name: 'shows when a sealed payload has no readable page.',
  render: () =>
    artifactStateView({
      selectedPayload: {
        payloadId: '00000000-0000-4000-8000-000000000004',
        label: '入力',
        pointer: '/records',
      },
      payloadRead: {
        status: 'ready',
        manifest: {
          status: 'sealed',
          itemCount: 0,
          incompleteReason: null,
        },
        page: null,
      },
    }),
}

export const Failed: Story = {
  name: 'shows an error summary for a task that failed during item checks.',
  render: () => (
    <RouterProvider
      router={createTaskRunViewRouter({
        strategyId: 'sample-strategy',
        task: baseTask({
          phase: 'failed',
          updatedAt: '2026-08-16T09:01:18.300Z',
          errorSummary: 'tool call timeout: item_lookup がタイムアウトしました',
        }),
        steps: [
          PREPARATION_STEP,
          {
            ...inspectStep('注記の抜け', { status: 'failed' }),
            finished_at: '2026-08-16T09:01:18.300Z',
            error: 'tool call timeout: item_lookup がタイムアウトしました',
            output: undefined,
          },
        ],
        configPhases: CONFIG_PHASES,
        notes: [],
        notesState: 'success',
      })}
    />
  ),
}

export const CompletedWithoutNotes: Story = {
  name: 'shows a completed task with no generated notes.',
  render: () => (
    <RouterProvider
      router={createTaskRunViewRouter({
        strategyId: 'sample-strategy',
        task: baseTask({
          phase: 'completed',
          updatedAt: '2026-08-16T09:04:33.200Z',
        }),
        steps: [PREPARATION_STEP],
        configPhases: CONFIG_PHASES,
        notes: [],
        notesState: 'success',
      })}
    />
  ),
}

export const NoSteps: Story = {
  name: 'shows a pending task before its execution steps are available.',
  render: () => (
    <RouterProvider
      router={createTaskRunViewRouter({
        strategyId: 'sample-strategy',
        task: baseTask({ phase: 'pending' }),
        steps: [],
        configPhases: [],
        notes: [],
        notesState: 'loading',
      })}
    />
  ),
}

export const Loading: Story = {
  name: 'shows a loading placeholder before task details are available.',
  render: () => (
    <RouterProvider
      router={createTaskRunViewRouter({
        strategyId: 'sample-strategy',
        task: null,
        steps: [],
        configPhases: [],
        notes: [],
        notesState: 'loading',
      })}
    />
  ),
}
