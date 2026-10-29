import {
  createMemoryHistory,
  createRootRoute,
  createRoute,
  createRouter,
  RouterProvider,
} from '@tanstack/react-router'
import {
  cleanup,
  render,
  screen,
  waitFor,
  within,
} from '@testing-library/react'
import { afterEach, describe, expect, it } from 'vitest'

import { DashboardRecentRuns } from '#components/dashboard/dashboard-recent-runs'
import type { components } from '#lib/api/schema.gen'

afterEach(cleanup)

type Task = components['schemas']['StrategyTaskSummary']

async function renderRecentRuns(tasks: Task[]) {
  const rootRoute = createRootRoute({
    component: () => (
      <DashboardRecentRuns tasks={tasks} strategies={[]} isError={false} />
    ),
  })
  const routesRoute = createRoute({
    getParentRoute: () => rootRoute,
    path: '/runs',
    component: () => null,
  })
  const detailRoute = createRoute({
    getParentRoute: () => rootRoute,
    path: '/strategies/$id/runs/$taskId',
    component: () => null,
  })
  const router = createRouter({
    routeTree: rootRoute.addChildren([routesRoute, detailRoute]),
    history: createMemoryHistory({ initialEntries: ['/'] }),
  })

  render(<RouterProvider router={router} />)
  await waitFor(() => {
    expect(
      document.body.firstElementChild?.children.length ?? 0,
    ).toBeGreaterThan(0)
  })
}

function readTaskBadgeRows(rows: HTMLElement[]) {
  return rows.map((row) => ({
    taskId: row.getAttribute('href')?.split('/').at(-1),
    badges: within(row)
      .queryAllByText('モデル呼び出し上限で打ち切り')
      .map((element) => element.textContent),
  }))
}

function makeTask(
  taskId: string,
  prompt: string,
  modelCallLimitReached: boolean,
  createdAt: string,
): Task {
  return {
    task_id: taskId,
    strategy_id: 'synthetic-strategy',
    source: 'frontend',
    prompt,
    phase: 'completed',
    purpose: null,
    as_of: null,
    error_summary: null,
    model_call_limit_reached: modelCallLimitReached,
    created_at: createdAt,
    updated_at: createdAt,
  }
}

describe('DashboardRecentRuns', () => {
  it('model call limit marker は印のある task にだけ表示する', async () => {
    await renderRecentRuns([
      makeTask(
        'synthetic-task-a',
        '架空の項目を確認する',
        true,
        '2030-01-02T03:05:05Z',
      ),
      makeTask(
        'synthetic-task-b',
        '別の架空項目を確認する',
        false,
        '2030-01-02T03:04:05Z',
      ),
    ])
    const limitedTask = screen.getByRole('link', {
      name: /架空の項目を確認する/,
    })
    const regularTask = screen.getByRole('link', {
      name: /別の架空項目を確認する/,
    })

    expect(readTaskBadgeRows([limitedTask, regularTask])).toEqual([
      { taskId: 'synthetic-task-a', badges: ['モデル呼び出し上限で打ち切り'] },
      { taskId: 'synthetic-task-b', badges: [] },
    ])
  })
})
