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
import userEvent from '@testing-library/user-event'
import type { ComponentProps } from 'react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { TaskRunDefinitionSnapshotPanel } from '#components/strategy-shell/task-run-definition-snapshot-panel'
import { TaskRunPayloadPanel } from '#components/strategy-shell/task-run-payload-panel'
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
    nextPage:
      Array.from(section.querySelectorAll('button')).find((button) =>
        button.textContent.includes('次のページを表示'),
      )?.textContent ?? null,
  }
}

function readDefinitionSnapshotOutput() {
  const section = screen.getByRole('region', { name: '実行時の固定定義' })
  return {
    selectedKey: screen.getByRole('combobox', { name: '固定定義 snapshot' })
      .textContent,
    metadata: Array.from(
      section.querySelectorAll(
        '[data-testid="definition-snapshot-metadata"] dt, [data-testid="definition-snapshot-metadata"] dd',
      ),
    ).map((element) => element.textContent),
    definition: section.querySelector('pre')?.textContent ?? null,
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

  it('ネスト配列 pointer を適用すると cursor を先頭に戻して bounded page を選び直す', async () => {
    const user = userEvent.setup()
    const onSelectPayload = vi.fn()
    await renderTaskView(makeTask(), [], 'success', {
      selectedPayload: {
        payloadId: '00000000-0000-4000-8000-000000000003',
        label: '出力',
        pointer: '/value',
        cursor: 8,
        limit: 2,
      },
      payloadRead: {
        status: 'ready',
        manifest: {
          status: 'sealed',
          itemCount: 3,
          incompleteReason: null,
        },
        page: {
          data: { items: [{ value: 'synthetic-a' }, { value: 'synthetic-b' }] },
          totalCount: 1,
          nextCursor: null,
        },
      },
      onSelectPayload,
    })

    await user.clear(screen.getByRole('textbox', { name: 'JSON Pointer' }))
    await user.type(
      screen.getByRole('textbox', { name: 'JSON Pointer' }),
      '/value/items',
    )
    await user.click(screen.getByRole('button', { name: 'pointer を適用' }))

    expect(onSelectPayload.mock.calls).toEqual([
      [
        {
          payloadId: '00000000-0000-4000-8000-000000000003',
          label: '出力',
          pointer: '/value/items',
          cursor: 0,
          limit: 2,
        },
      ],
    ])
  })

  it('保存済み definition_snapshots から custom_indicator を選んで本文と metadata を表示する', async () => {
    const user = userEvent.setup()
    await renderTaskView(makeTask(), [], 'success', {
      definitionSnapshotsState: 'ready',
      definitionSnapshots: [
        {
          definitionKey: 'task_start',
          taskId: 'sample-task',
          digest: 'synthetic-task-digest',
          createdAt: '2026-08-15T00:00:00.000Z',
          definition: { config: 'synthetic task definition' },
        },
        {
          definitionKey: 'custom_indicator:sample-indicator',
          taskId: 'sample-task',
          digest: 'synthetic-indicator-digest',
          createdAt: '2026-08-15T00:00:01.000Z',
          definition: {
            name: 'sample-indicator',
            code: 'synthetic code body',
            metadata: { revision: 'synthetic-r1' },
          },
        },
      ],
    })

    await user.click(
      screen.getByRole('combobox', { name: '固定定義 snapshot' }),
    )
    await user.click(
      await screen.findByRole('option', {
        name: 'custom_indicator:sample-indicator',
      }),
    )
    await user.click(screen.getByText('固定定義本文を表示'))

    expect(readDefinitionSnapshotOutput()).toEqual({
      selectedKey: 'custom_indicator:sample-indicator',
      metadata: [
        '定義キー',
        'custom_indicator:sample-indicator',
        'SHA-256',
        'synthetic-indicator-digest',
        '記録日時',
        '2026-08-15T00:00:01.000Z',
        'task ID',
        'sample-task',
      ],
      definition:
        '{\n  "name": "sample-indicator",\n  "code": "synthetic code body",\n  "metadata": {\n    "revision": "synthetic-r1"\n  }\n}',
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

describe('TaskRunDefinitionSnapshotPanel', () => {
  it('initialSelectedKey で選んだ固定定義の本文を開いて表示する', () => {
    render(
      <TaskRunDefinitionSnapshotPanel
        snapshots={[
          {
            definitionKey: 'task_start',
            taskId: 'sample-task',
            digest: 'synthetic-task-digest',
            createdAt: '2026-08-15T00:00:00.000Z',
            definition: { config: 'synthetic task definition' },
          },
          {
            definitionKey: 'custom_indicator:sample-indicator',
            taskId: 'sample-task',
            digest: 'synthetic-indicator-digest',
            createdAt: '2026-08-15T00:00:01.000Z',
            definition: {
              name: 'sample-indicator',
              code: 'synthetic code body',
              metadata: { revision: 'synthetic-r1' },
            },
          },
        ]}
        state="ready"
        initialSelectedKey="custom_indicator:sample-indicator"
        definitionBodyOpen
      />,
    )

    expect(readDefinitionSnapshotOutput()).toEqual({
      selectedKey: 'custom_indicator:sample-indicator',
      metadata: [
        '定義キー',
        'custom_indicator:sample-indicator',
        'SHA-256',
        'synthetic-indicator-digest',
        '記録日時',
        '2026-08-15T00:00:01.000Z',
        'task ID',
        'sample-task',
      ],
      definition:
        '{\n  "name": "sample-indicator",\n  "code": "synthetic code body",\n  "metadata": {\n    "revision": "synthetic-r1"\n  }\n}',
    })
  })
})

describe('TaskRunPayloadPanel', () => {
  it('initialPointerDraft を表示して適用時に先頭 cursor を選ぶ', async () => {
    const user = userEvent.setup()
    const onSelectPayload = vi.fn()
    render(
      <TaskRunPayloadPanel
        reference={{
          payloadId: '00000000-0000-4000-8000-000000000004',
          label: '出力',
          pointer: '/value',
          cursor: 8,
          limit: 2,
        }}
        initialPointerDraft="/value/items"
        payloadRead={{
          status: 'ready',
          manifest: {
            status: 'sealed',
            itemCount: 10,
            incompleteReason: null,
          },
          page: {
            data: { items: ['synthetic-a', 'synthetic-b'] },
            totalCount: 1,
            nextCursor: null,
          },
        }}
        onSelectPayload={onSelectPayload}
      />,
    )

    expect(screen.getByRole('textbox', { name: 'JSON Pointer' })).toHaveValue(
      '/value/items',
    )
    await user.click(screen.getByRole('button', { name: 'pointer を適用' }))

    expect(onSelectPayload.mock.calls).toEqual([
      [
        {
          payloadId: '00000000-0000-4000-8000-000000000004',
          label: '出力',
          pointer: '/value/items',
          cursor: 0,
          limit: 2,
        },
      ],
    ])
  })
})
