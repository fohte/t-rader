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
      <div className="min-h-screen bg-[color:var(--color-bg-primary)] p-6">
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
  render: () => (
    <RouterProvider
      router={createTaskRunListViewRouter({
        strategyId: 'semi-swing',
        tasks: [
          {
            taskId: 'task-0003',
            prompt: '半導体セクターの調整は一時的か、循環の転換点か',
            source: 'frontend',
            phase: 'running',
            createdAt: '2026-08-16T09:00:00.000Z',
          },
          {
            taskId: 'task-0002',
            prompt: '円安進行を受けたポジション調整の要否を確認',
            source: 'cron',
            phase: 'completed',
            createdAt: '2026-08-15T21:00:00.000Z',
          },
          {
            taskId: 'task-0001',
            prompt: 'SUMCO のレンジ回帰シナリオを再評価',
            source: 'mgmt-mcp',
            phase: 'failed',
            createdAt: '2026-08-14T03:00:00.000Z',
          },
        ],
      })}
    />
  ),
}

export const Loading: Story = {
  render: () => (
    <RouterProvider
      router={createTaskRunListViewRouter({
        strategyId: 'semi-swing',
        tasks: null,
      })}
    />
  ),
}

export const Empty: Story = {
  render: () => (
    <RouterProvider
      router={createTaskRunListViewRouter({
        strategyId: 'semi-swing',
        tasks: [],
      })}
    />
  ),
}
