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
  additionalProps: Partial<ComponentProps<typeof TaskRunView>> = {},
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
        {...additionalProps}
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

function readPayloadPanelOutput(section: HTMLElement) {
  return {
    title: section.querySelector('h2')?.textContent,
    messages: Array.from(section.querySelectorAll('p')).map((paragraph) =>
      paragraph.textContent.trim(),
    ),
    content: section.querySelector('pre')?.textContent ?? null,
    nextPage: section.querySelector('button')?.textContent ?? null,
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

  it('payload page の全件数と表示件数、次ページを区別する', async () => {
    await renderTaskView(makeTask(), [], 'success', {
      selectedPayload: {
        payloadId: '00000000-0000-4000-8000-000000000001',
        label: 'input: source',
        pointer: '/records',
        cursor: 0,
        limit: 2,
      },
      payloadRead: {
        status: 'ready',
        manifest: {
          status: 'sealed',
          itemCount: 5,
          incompleteReason: null,
        },
        page: {
          data: [{ value: 'sample-a' }, { value: 'sample-b' }],
          totalCount: 5,
          nextCursor: 2,
        },
      },
      onNextPayloadPage: () => {},
    })

    const section = screen.getByRole('region', { name: '実行 payload' })
    expect(readPayloadPanelOutput(section)).toEqual({
      title: 'input: source',
      messages: [
        '00000000-0000-4000-8000-000000000001 · pointer /records',
        '全 5 件 · 表示 2 件',
      ],
      content:
        '[\n  {\n    "value": "sample-a"\n  },\n  {\n    "value": "sample-b"\n  }\n]',
      nextPage: '次のページを表示',
    })
  })

  it('未完了 payload の理由だけを表示し内容を表示しない', async () => {
    await renderTaskView(makeTask(), [], 'success', {
      selectedPayload: {
        payloadId: '00000000-0000-4000-8000-000000000002',
        label: '出力',
        pointer: '/value',
      },
      payloadRead: {
        status: 'ready',
        manifest: {
          status: 'incomplete',
          itemCount: 0,
          incompleteReason: 'capture interrupted',
        },
        page: null,
      },
    })

    const section = screen.getByRole('region', { name: '実行 payload' })
    expect(readPayloadPanelOutput(section)).toEqual({
      title: '出力',
      messages: [
        '00000000-0000-4000-8000-000000000002 · pointer /value',
        'payload は未完了のため内容を表示できません。 capture interrupted',
      ],
      content: null,
      nextPage: null,
    })
  })
})
