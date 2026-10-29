import type { Meta, StoryObj } from '@storybook/react-vite'
import { RouterProvider } from '@tanstack/react-router'
import type { ComponentType } from 'react'

import { DashboardRecentRuns } from '#components/dashboard/dashboard-recent-runs'
import type { components } from '#lib/api/schema.gen'
import { createStoryRouter } from '#storybook/story-router'

type Strategy = components['schemas']['Strategy']
type Task = components['schemas']['StrategyTaskSummary']

const strategies: Strategy[] = [
  {
    id: 'fake-strategy-a',
    name: 'デモ戦略',
    description: null,
    sort_order: 0,
    created_at: '2026-01-01T00:00:00Z',
    updated_at: '2026-01-01T00:00:00Z',
  },
]
const tasks: Task[] = [
  {
    task_id: 'fake-task-a',
    strategy_id: 'fake-strategy-a',
    source: 'frontend',
    prompt: '決算前の確認',
    phase: 'running',
    purpose: null,
    as_of: null,
    error_summary: null,
    model_call_limit_reached: false,
    created_at: '2026-10-10T03:00:00Z',
    updated_at: '2026-10-10T03:00:00Z',
  },
]

function withRouter(Story: ComponentType) {
  return (
    <RouterProvider
      router={createStoryRouter(
        () => (
          <Story />
        ),
        {
          paths: ['/runs', '/strategies/$id/runs/$taskId'],
        },
      )}
    />
  )
}

const meta = {
  title: 'Dashboard/DashboardRecentRuns',
  component: DashboardRecentRuns,
  decorators: [withRouter],
  args: { tasks, strategies, isError: false },
} satisfies Meta<typeof DashboardRecentRuns>

export default meta
type Story = StoryObj<typeof meta>

export const Recent: Story = {
  name: 'shows recent task runs with their phase and strategy.',
}

export const Empty: Story = {
  name: 'shows an empty state when no task has run.',
  args: { tasks: [] },
}

export const Loading: Story = {
  name: 'shows placeholders while task runs are loading.',
  args: { tasks: null },
}

export const Error: Story = {
  name: 'shows an error message when task runs cannot be loaded.',
  args: { tasks: null, isError: true },
}
