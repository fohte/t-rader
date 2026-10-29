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
            strategyId: 'sample-strategy',
            strategyName: 'サンプル戦略',
            prompt: 'サンプル項目の記録内容を確認する',
            source: 'frontend',
            phase: 'running',
            purpose: 'inspect',
            createdAt: '2026-08-16T09:00:00.000Z',
            modelCallLimitReached: false,
          },
          {
            taskId: 'task-0002',
            strategyId: 'sample-strategy',
            prompt: '項目一覧の表記を確認する',
            source: 'cron',
            phase: 'completed',
            purpose: null,
            createdAt: '2026-08-15T21:00:00.000Z',
            modelCallLimitReached: false,
          },
          {
            taskId: 'task-0001',
            strategyId: 'sample-strategy',
            prompt: '重複している項目を確認する',
            source: 'mgmt-mcp',
            phase: 'failed',
            purpose: null,
            createdAt: '2026-08-14T03:00:00.000Z',
            modelCallLimitReached: false,
          },
        ],
      })}
    />
  ),
}

export const WithModelCallLimitReached: Story = {
  name: 'shows the limit badge only on marked tasks.',
  render: () => (
    <RouterProvider
      router={createTaskRunListViewRouter({
        tasks: [
          {
            taskId: 'synthetic-task-a',
            strategyId: 'synthetic-strategy',
            strategyName: 'デモ',
            prompt: '架空の項目を確認する',
            source: 'frontend',
            phase: 'completed',
            purpose: null,
            createdAt: '2030-01-02T03:04:05.000Z',
            modelCallLimitReached: true,
          },
          {
            taskId: 'synthetic-task-b',
            strategyId: 'synthetic-strategy',
            strategyName: 'デモ',
            prompt: '別の架空項目を確認する',
            source: 'frontend',
            phase: 'completed',
            purpose: null,
            createdAt: '2030-01-02T03:05:05.000Z',
            modelCallLimitReached: false,
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
  name: 'shows the empty state when the strategy has no task runs.',
  render: () => (
    <RouterProvider
      router={createTaskRunListViewRouter({
        tasks: [],
      })}
    />
  ),
}
