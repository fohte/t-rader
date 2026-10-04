import {
  createMemoryHistory,
  createRootRoute,
  createRoute,
  createRouter,
  RouterProvider,
} from '@tanstack/react-router'
import { cleanup, render, screen, waitFor } from '@testing-library/react'
import type { ComponentProps } from 'react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import {
  sourceLabel,
  TaskRunView,
} from '#components/strategy-shell/task-run-view'

afterEach(() => {
  cleanup()
  vi.useRealTimers()
})

type TaskRun = NonNullable<ComponentProps<typeof TaskRunView>['task']>

function makeTask(overrides: Partial<TaskRun> = {}): TaskRun {
  return {
    taskId: 'task-a',
    prompt: '調査依頼',
    source: 'frontend',
    phase: 'completed',
    createdAt: '2026-08-15T00:00:00.000Z',
    updatedAt: '2026-08-15T00:01:05.000Z',
    errorSummary: null,
    ...overrides,
  }
}

async function renderTaskView(task: TaskRun) {
  const rootRoute = createRootRoute({
    component: () => (
      <TaskRunView
        strategyId="strategy-a"
        task={task}
        steps={[]}
        configPhases={[]}
        generatedNotesCount={0}
      />
    ),
  })
  const runsRoute = createRoute({
    getParentRoute: () => rootRoute,
    path: '/strategies/$id/runs',
    component: () => null,
  })
  const router = createRouter({
    routeTree: rootRoute.addChildren([runsRoute]),
    history: createMemoryHistory({ initialEntries: ['/'] }),
  })
  render(<RouterProvider router={router} />)
  await waitFor(() => {
    expect(
      document.body.firstElementChild?.children.length ?? 0,
    ).toBeGreaterThan(0)
  })
}

function readElapsedText(): string | null {
  const summary = screen.getByText(
    (_, element) =>
      element?.tagName === 'P' && element.textContent.includes(' · 経過 '),
  )
  return summary.textContent.split(' · 経過 ').at(-1) ?? null
}

describe('sourceLabel', () => {
  it.each([
    ['frontend', 'フローティングチャット'],
    ['unknown-source', 'unknown-source'],
  ])('%s を %s に変換する', (source, expected) => {
    expect(sourceLabel(source)).toBe(expected)
  })
})

describe('TaskRunView', () => {
  it.each([
    [
      'completed',
      '2026-08-15T00:00:00.000Z',
      '2026-08-15T00:00:59.000Z',
      '59s',
    ],
    [
      'completed',
      '2026-08-15T00:00:00.000Z',
      '2026-08-15T00:01:05.000Z',
      '1m5s',
    ],
    ['completed', '2026-08-15T00:00:00.000Z', '2026-08-15T00:00:00.000Z', '0s'],
    ['failed', '2026-08-15T00:00:00.000Z', '2026-08-15T00:00:59.000Z', '59s'],
  ] as const)(
    'shows the elapsed time for a %s task',
    async (phase, createdAt, updatedAt, expected) => {
      await renderTaskView(makeTask({ phase, createdAt, updatedAt }))
      expect(readElapsedText()).toBe(expected)
    },
  )

  it('uses the current time for a running task', async () => {
    vi.useFakeTimers({ toFake: ['Date'] })
    vi.setSystemTime(new Date('2026-08-15T00:01:05.000Z'))

    await renderTaskView(makeTask({ phase: 'running' }))

    expect(readElapsedText()).toBe('1m5s')
  })
})
