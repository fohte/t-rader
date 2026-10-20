import type { Meta, StoryObj } from '@storybook/react-vite'
import { RouterProvider } from '@tanstack/react-router'

import {
  TaskRunListView,
  type TaskRunListViewProps,
} from '#components/strategy-shell/task-run-list-view'
import { createStoryRouter } from '#storybook/story-router'

function createTaskRunListViewRouter(props: TaskRunListViewProps) {
  return createStoryRouter(
    () => (
      <div className="min-h-screen bg-background p-6">
        <TaskRunListView {...props} />
      </div>
    ),
    { paths: ['/strategies/$id/runs/$taskId'] },
  )
}

const meta = {
  title: 'StrategyShell/TaskRunListView',
  parameters: { layout: 'fullscreen' },
} satisfies Meta

export default meta
type Story = StoryObj<typeof meta>

export const List: Story = {
  name: 'shows running, completed, and failed tasks in the run history.',
  render: () => (
    <RouterProvider
      router={createTaskRunListViewRouter({
        tasks: [
          {
            taskId: 'task-0003',
            strategyId: 'sample-workspace',
            strategyName: 'サンプルワークスペース',
            prompt: 'サンプル項目の記録内容を確認する',
            source: 'frontend',
            phase: 'running',
            purpose: 'inspect',
            createdAt: '2026-08-16T09:00:00.000Z',
          },
          {
            taskId: 'task-0002',
            strategyId: 'sample-workspace',
            prompt: '項目一覧の表記を確認する',
            source: 'cron',
            phase: 'completed',
            purpose: null,
            createdAt: '2026-08-15T21:00:00.000Z',
          },
          {
            taskId: 'task-0001',
            strategyId: 'sample-workspace',
            prompt: '重複している項目を確認する',
            source: 'mgmt-mcp',
            phase: 'failed',
            purpose: null,
            createdAt: '2026-08-14T03:00:00.000Z',
          },
        ],
      })}
    />
  ),
}

export const Loading: Story = {
  name: 'shows the loading state while task runs are being fetched.',
  render: () => (
    <RouterProvider
      router={createTaskRunListViewRouter({
        tasks: null,
      })}
    />
  ),
}

export const Empty: Story = {
  name: 'shows the empty state when the workspace has no task runs.',
  render: () => (
    <RouterProvider
      router={createTaskRunListViewRouter({
        tasks: [],
      })}
    />
  ),
}
