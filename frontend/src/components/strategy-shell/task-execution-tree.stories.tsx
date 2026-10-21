import type { Meta, StoryObj } from '@storybook/react-vite'
import { RouterProvider } from '@tanstack/react-router'

import {
  TaskExecutionTree,
  type TaskStep,
} from '#components/strategy-shell/task-execution-tree'
import {
  CONFIG_PHASES,
  inspectStep,
  PREPARATION_STEP,
} from '#components/strategy-shell/task-execution-tree.fixtures'
import { createStoryRouter } from '#storybook/story-router'

// ノートリンクが親ルートを要求するため、Frame 自体をルーターで包む
function Frame({ children }: { children: React.ReactNode }) {
  const router = createStoryRouter(
    () => (
      <div className="min-h-screen bg-background p-6">
        <div className="w-105 border border-border bg-bg-secondary p-3.5">
          {children}
        </div>
      </div>
    ),
    { paths: ['/notes/$noteId'] },
  )
  return <RouterProvider router={router} />
}

const meta = {
  title: 'StrategyShell/TaskExecutionTree',
  parameters: { layout: 'fullscreen' },
} satisfies Meta

export default meta
type Story = StoryObj<typeof meta>

const EXPANDED_STEP: TaskStep = {
  phase_key: 'sample-phase',
  execution_step_id: 'sample-step-001',
  label: 'サンプル処理',
  model: 'simulated-model-cobalt',
  status: 'completed',
  item: { record: 'sample-row' },
  item_label: 'サンプル項目',
  output: { state: 'sample-result', note_id: 'sample-note-001' },
  started_at: '2026-08-15T09:00:00.000Z',
  finished_at: '2026-08-15T09:00:08.000Z',
  trace_id: 'sample-trace-001',
  span_id: 'sample-span-001',
}

// 結果整理フェーズには step を渡さないので、常に「未着手」ノードとして描画される
export const Running: Story = {
  name: 'shows an active task with completed and running item checks.',
  render: () => (
    <Frame>
      <TaskExecutionTree
        steps={[
          PREPARATION_STEP,
          inspectStep('見出しの表記ゆれ', {
            status: 'completed',
            finishedAt: '2026-08-15T09:00:21.100Z',
            result: 'consistent',
            noteId: 'note-0001',
          }),
          inspectStep('注記の抜け', { status: 'running' }),
          inspectStep('項目間の重複', {
            status: 'completed',
            finishedAt: '2026-08-15T09:00:19.700Z',
            result: 'needs_review',
          }),
        ]}
        configPhases={CONFIG_PHASES}
        traceUrlTemplate="https://grafana.example/explore?traceID={trace_id}&spanID={span_id}"
      />
    </Frame>
  ),
}

export const Completed: Story = {
  name: 'shows a finished run with review results and generated notes.',
  render: () => (
    <Frame>
      <TaskExecutionTree
        steps={[
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
          inspectStep('項目間の重複', {
            status: 'completed',
            finishedAt: '2026-08-15T09:00:19.700Z',
            result: 'undetermined',
          }),
          {
            phase_key: 'finalize',
            execution_step_id: 'exec-finalize-0001',
            label: '結果整理',
            model: 'simulated-model-violet',
            status: 'completed',
            output: {
              details: '見出しの表記ゆれと項目間の重複を確認',
              note_id: 'note-0003',
            },
            started_at: '2026-08-15T09:00:25.000Z',
            finished_at: '2026-08-15T09:00:33.200Z',
            trace_id: 'trace-finalize-0001',
            span_id: 'span-finalize-0001',
          },
        ]}
        configPhases={CONFIG_PHASES}
        traceUrlTemplate="https://grafana.example/explore?traceID={trace_id}&spanID={span_id}"
      />
    </Frame>
  ),
}

export const CompletedWithNoEntries: Story = {
  name: 'shows a completed phase when it has no entries.',
  render: () => (
    <Frame>
      <TaskExecutionTree
        steps={[
          {
            phase_key: 'source',
            execution_step_id: 'source-step',
            label: '入力',
            model: 'simulated-model-amber',
            status: 'completed',
            output: { entries: [] },
            started_at: '2026-08-15T09:00:00.000Z',
            finished_at: '2026-08-15T09:00:01.000Z',
            trace_id: 'source-trace',
            span_id: 'source-span',
          },
          {
            phase_key: 'summary',
            execution_step_id: 'summary-step',
            label: '整理',
            model: 'simulated-model-violet',
            status: 'completed',
            output: { details: 'No entries' },
            started_at: '2026-08-15T09:00:01.100Z',
            finished_at: '2026-08-15T09:00:02.000Z',
            trace_id: 'summary-trace',
            span_id: 'summary-span',
          },
        ]}
        configPhases={[
          { key: 'source', label: '入力', model: 'simulated-model-amber' },
          {
            key: 'inspect',
            label: '確認',
            model: 'simulated-model-teal',
            forEach: 'source.entries',
          },
          { key: 'summary', label: '整理', model: 'simulated-model-violet' },
        ]}
        taskCompleted
      />
    </Frame>
  ),
}

export const WithFailure: Story = {
  name: 'shows a failed item check alongside completed checks.',
  render: () => (
    <Frame>
      <TaskExecutionTree
        steps={[
          PREPARATION_STEP,
          inspectStep('見出しの表記ゆれ', {
            status: 'completed',
            finishedAt: '2026-08-15T09:00:21.100Z',
            result: 'consistent',
          }),
          {
            ...inspectStep('注記の抜け', { status: 'failed' }),
            finished_at: '2026-08-15T09:00:18.300Z',
            error: 'tool call timeout: item_lookup がタイムアウトしました',
            output: undefined,
          },
          inspectStep('項目間の重複', {
            status: 'completed',
            finishedAt: '2026-08-15T09:00:19.700Z',
            result: 'needs_review',
          }),
        ]}
        configPhases={CONFIG_PHASES}
      />
    </Frame>
  ),
}

export const Expanded: Story = {
  name: 'the selected step shows its details.',
  render: () => (
    <Frame>
      <TaskExecutionTree
        steps={[EXPANDED_STEP]}
        configPhases={[
          {
            key: 'sample-phase',
            label: 'サンプル処理',
            model: 'simulated-model-cobalt',
            output: { state: { enum: ['sample-result'] } },
          },
        ]}
        defaultSelectedStepId={EXPANDED_STEP.execution_step_id}
        traceUrlTemplate="https://example.test/trace/{trace_id}?span={span_id}"
      />
    </Frame>
  ),
}

export const NoAgentGraph: Story = {
  name: 'shows no execution steps when no agent graph is configured.',
  render: () => (
    <Frame>
      <p className="font-mono text-2xs text-muted-foreground">
        agent_graph 未設定 (steps が空) — 以下、何も表示されません:
      </p>
      <TaskExecutionTree steps={[]} configPhases={[]} />
    </Frame>
  ),
}
