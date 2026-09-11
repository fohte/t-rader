import {
  createMemoryHistory,
  createRootRoute,
  createRoute,
  createRouter,
  RouterProvider,
} from '@tanstack/react-router'
import { cleanup, render, screen, waitFor } from '@testing-library/react'
import { afterEach, describe, expect, it } from 'vitest'

import {
  type TaskRunListItem,
  TaskRunListView,
} from '#components/strategy-shell/task-run-list-view'

afterEach(cleanup)

// Link が親ルートを要求するため、最低限のテストルーターを噛ませる
async function renderInRouter(tasks: TaskRunListItem[]) {
  const rootRoute = createRootRoute({
    component: () => <TaskRunListView tasks={tasks} />,
  })
  const detailRoute = createRoute({
    getParentRoute: () => rootRoute,
    path: '/strategies/$id/runs/$taskId',
    component: () => null,
  })
  const router = createRouter({
    routeTree: rootRoute.addChildren([detailRoute]),
    history: createMemoryHistory({ initialEntries: ['/'] }),
  })
  render(<RouterProvider router={router} />)
  await waitFor(() => {
    expect(
      document.body.firstElementChild?.children.length ?? 0,
    ).toBeGreaterThan(0)
  })
}

describe('TaskRunListView', () => {
  it('未知の phase はそのまま表示する', async () => {
    await renderInRouter([
      {
        taskId: 'task-1',
        strategyId: 'strategy-1',
        prompt: 'p',
        source: 'frontend',
        phase: 'unknown-phase',
        purpose: null,
        createdAt: '2026-08-15T00:00:00.000Z',
      },
    ])
    expect(screen.getByText('unknown-phase')).toBeInTheDocument()
  })

  it('strategyId ごとにリンク先が変わる', async () => {
    await renderInRouter([
      {
        taskId: 't1',
        strategyId: 'strategy-a',
        prompt: 'a',
        source: 'frontend',
        phase: 'completed',
        purpose: null,
        createdAt: '2026-08-15T00:00:00.000Z',
      },
      {
        taskId: 't2',
        strategyId: 'strategy-b',
        prompt: 'b',
        source: 'frontend',
        phase: 'completed',
        purpose: null,
        createdAt: '2026-08-15T00:00:00.000Z',
      },
    ])
    expect(screen.getByRole('link', { name: /a/ })).toHaveAttribute(
      'href',
      '/strategies/strategy-a/runs/t1',
    )
    expect(screen.getByRole('link', { name: /b/ })).toHaveAttribute(
      'href',
      '/strategies/strategy-b/runs/t2',
    )
  })
})
