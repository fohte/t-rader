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

import { TaskRunNotes } from '#components/strategy-shell/task-run-notes'

afterEach(cleanup)

type TaskRunNote = ComponentProps<typeof TaskRunNotes>['notes'][number]
type TaskRunNotesState = ComponentProps<typeof TaskRunNotes>['state']

async function renderTaskRunNotes(
  state: TaskRunNotesState,
  notes: TaskRunNote[] = [],
) {
  const rootRoute = createRootRoute({
    component: () => <TaskRunNotes notes={notes} state={state} />,
  })
  const noteRoute = createRoute({
    getParentRoute: () => rootRoute,
    path: '/notes/$noteId',
    component: () => null,
  })
  const router = createRouter({
    routeTree: rootRoute.addChildren([noteRoute]),
    history: createMemoryHistory({ initialEntries: ['/'] }),
  })
  render(<RouterProvider router={router} />)
  await waitFor(() => {
    expect(
      document.body.firstElementChild?.children.length ?? 0,
    ).toBeGreaterThan(0)
  })
}

function readTaskRunNotesOutput() {
  const region = screen.getByRole('region', {
    name: '作成・更新したノート',
  })
  const queries = within(region)
  return {
    heading: queries.getByRole('heading', { level: 2 }).textContent,
    count: queries.getByText(/^(\d+ 件|取得中|取得失敗)$/).textContent,
    message:
      queries.queryByRole('status')?.textContent ??
      queries.queryByRole('alert')?.textContent ??
      queries.queryByText('この実行で作成・更新されたノートはありません。')
        ?.textContent ??
      null,
    notes: queries.queryAllByRole('link').map((link) => ({
      title: link.textContent,
      href: link.getAttribute('href'),
    })),
  }
}

describe('TaskRunNotes', () => {
  it('ノート一覧と各ノートの詳細リンクを表示する', async () => {
    await renderTaskRunNotes('success', [
      { id: 'fake-note-001', title: 'サンプル調査メモ' },
      { id: 'fake-note-002', title: '架空グループの確認事項' },
    ])

    expect(readTaskRunNotesOutput()).toEqual({
      heading: '> 作成・更新したノート',
      count: '2 件',
      message: null,
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
  }, 10_000)

  it('ノートが 0 件のときに空表示を出す', async () => {
    await renderTaskRunNotes('success')

    expect(readTaskRunNotesOutput()).toEqual({
      heading: '> 作成・更新したノート',
      count: '0 件',
      message: 'この実行で作成・更新されたノートはありません。',
      notes: [],
    })
  })

  it('再取得に失敗しても前回取得したノートを表示する', async () => {
    await renderTaskRunNotes('error', [
      { id: 'fake-note-001', title: '前回取得した調査メモ' },
    ])

    expect(readTaskRunNotesOutput()).toEqual({
      heading: '> 作成・更新したノート',
      count: '1 件',
      message: 'ノートの取得に失敗しました。',
      notes: [
        {
          title: '前回取得した調査メモ',
          href: '/notes/fake-note-001',
        },
      ],
    })
  })

  it.each([
    ['loading', '取得中', 'ノートを読み込み中です。'],
    ['error', '取得失敗', 'ノートの取得に失敗しました。'],
  ] as const)(
    '%s のときに空表示ではなく状態を示す',
    async (state, count, message) => {
      await renderTaskRunNotes(state)

      expect(readTaskRunNotesOutput()).toEqual({
        heading: '> 作成・更新したノート',
        count,
        message,
        notes: [],
      })
    },
  )
})
