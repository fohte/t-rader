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
import type { ComponentProps } from 'react'
import { afterEach, describe, expect, it } from 'vitest'

import { TaskRunListView } from '#components/strategy-shell/task-run-list-view'

afterEach(cleanup)

type TaskRunListItem = NonNullable<
  ComponentProps<typeof TaskRunListView>['tasks']
>[number]

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

function readTaskBadgeRows(rows: HTMLElement[]) {
  return rows.map((row) => ({
    taskId: row.getAttribute('href')?.split('/').at(-1),
    badges: within(row)
      .queryAllByText('上限で打ち切り')
      .map((element) => element.textContent),
  }))
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
        modelCallLimitReached: false,
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
        modelCallLimitReached: false,
      },
      {
        taskId: 't2',
        strategyId: 'strategy-b',
        prompt: 'b',
        source: 'frontend',
        phase: 'completed',
        purpose: null,
        createdAt: '2026-08-15T00:00:00.000Z',
        modelCallLimitReached: false,
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

  it('model call limit marker は印のある task にだけ表示する', async () => {
    await renderInRouter([
      {
        taskId: 'synthetic-task-a',
        strategyId: 'synthetic-strategy',
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
        prompt: '別の架空項目を確認する',
        source: 'frontend',
        phase: 'completed',
        purpose: null,
        createdAt: '2030-01-02T03:05:05.000Z',
        modelCallLimitReached: false,
      },
    ])
    const limitedTask = screen.getByRole('link', {
      name: /架空の項目を確認する/,
    })
    const regularTask = screen.getByRole('link', {
      name: /別の架空項目を確認する/,
    })

    expect(readTaskBadgeRows([limitedTask, regularTask])).toEqual([
      { taskId: 'synthetic-task-a', badges: ['上限で打ち切り'] },
      { taskId: 'synthetic-task-b', badges: [] },
    ])
  })
})
