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
type TaskRunNote = ComponentProps<typeof TaskRunView>['notes'][number]

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

async function renderTaskView(
  task: TaskRun,
  notes: TaskRunNote[] = [],
  notesState: ComponentProps<typeof TaskRunView>['notesState'] = 'success',
) {
  const rootRoute = createRootRoute({
    component: () => (
      <TaskRunView
        strategyId="strategy-a"
        task={task}
        steps={[]}
        configPhases={[]}
        notes={notes}
        notesState={notesState}
      />
    ),
  })
  const runsRoute = createRoute({
    getParentRoute: () => rootRoute,
    path: '/strategies/$id/runs',
    component: () => null,
  })
  const noteRoute = createRoute({
    getParentRoute: () => rootRoute,
    path: '/notes/$noteId',
    component: () => null,
  })
  const router = createRouter({
    routeTree: rootRoute.addChildren([runsRoute, noteRoute]),
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

function readNotesOutput() {
  const section = screen.getByRole('region', {
    name: '作成・更新したノート',
  })
  const scopedQueries = within(section)
  return {
    heading: scopedQueries.getByRole('heading', { level: 2 }).textContent,
    count: scopedQueries.getByText(/^\d+ 件$/).textContent,
    emptyMessage:
      scopedQueries.queryByText(
        'この実行で作成・更新されたノートはありません。',
      )?.textContent ?? null,
    notes: scopedQueries.queryAllByRole('link').map((link) => ({
      title: link.textContent,
      href: link.getAttribute('href'),
    })),
  }
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

  it('ノート一覧に詳細へのリンクを表示する', async () => {
    await renderTaskView(makeTask(), [
      { id: 'fake-note-001', title: 'サンプル調査メモ' },
      { id: 'fake-note-002', title: '架空グループの確認事項' },
    ])

    expect(readNotesOutput()).toEqual({
      heading: '> 作成・更新したノート',
      count: '2 件',
      emptyMessage: null,
      notes: [
        {
          title: 'サンプル調査メモ',
          href: '/notes/fake-note-001',
        },
        {
          title: '架空グループの確認事項',
          href: '/notes/fake-note-002',
        },
      ],
    })
  })

  it('ノートが 0 件のときに空表示を出す', async () => {
    await renderTaskView(makeTask())

    expect(readNotesOutput()).toEqual({
      heading: '> 作成・更新したノート',
      count: '0 件',
      emptyMessage: 'この実行で作成・更新されたノートはありません。',
      notes: [],
    })
  })
})
