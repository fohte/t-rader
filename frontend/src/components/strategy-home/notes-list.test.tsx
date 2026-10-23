import {
  createMemoryHistory,
  createRootRoute,
  createRoute,
  createRouter,
  RouterProvider,
} from '@tanstack/react-router'
import { cleanup, render, screen, waitFor } from '@testing-library/react'
import { afterEach, describe, expect, it } from 'vitest'

import { NotesList } from '#components/strategy-home/notes-list'
import type { components } from '#lib/api/schema.gen'

type Note = components['schemas']['Note']
type NoteKind = components['schemas']['NoteKind']

const noteKinds: NoteKind[] = [
  {
    key: 'sample-kind',
    display_name: '架空の種別表示名',
    requires_approval: false,
    sort_order: 0,
  },
]

function makeNote(overrides: Partial<Note> = {}): Note {
  return {
    id: overrides.id ?? crypto.randomUUID(),
    version_id: overrides.version_id ?? crypto.randomUUID(),
    version_no: overrides.version_no ?? 1,
    is_current: overrides.is_current ?? true,
    title: overrides.title ?? 'title',
    body_md: overrides.body_md ?? 'body',
    frontmatter_json: overrides.frontmatter_json ?? {},
    resolved_price_references_json:
      overrides.resolved_price_references_json ?? {},
    graphs_json: overrides.graphs_json ?? [],
    tags: overrides.tags ?? [],
    kind: overrides.kind ?? null,
    status: overrides.status ?? 'unread',
    trigger: overrides.trigger ?? null,
    trigger_label: overrides.trigger_label ?? null,
    created_by_kind: overrides.created_by_kind ?? 'llm',
    created_at: overrides.created_at ?? '2026-01-01T00:00:00Z',
    updated_at: overrides.updated_at ?? '2026-01-01T00:00:00Z',
  }
}

afterEach(cleanup)

// Link が親ルートを要求するため、最低限のテストルーターを噛ませる
async function renderInRouter(
  notes: Note[],
  options: {
    noteKinds?: NoteKind[]
    noteKindCatalogState?: 'loading' | 'error' | 'loaded'
  } = {},
) {
  const rootRoute = createRootRoute({
    component: () => (
      <NotesList
        notes={notes}
        noteKinds={options.noteKinds ?? noteKinds}
        noteKindCatalogState={options.noteKindCatalogState ?? 'loaded'}
      />
    ),
  })
  const detailRoute = createRoute({
    getParentRoute: () => rootRoute,
    path: '/notes/$noteId',
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

function getNoteLinkSnapshot() {
  const link = screen.getByRole('link', { name: /口座全体ノート/ })
  const title = screen.getByText('口座全体ノート')
  return [title.textContent, link.getAttribute('href')]
}

function getKindLabelOutput(label: string, key: string) {
  return {
    kindLabel: screen.queryByText(label)?.textContent ?? null,
    rawKeyVisible: screen.queryByText(key) != null,
  }
}

function getLinkedKindRowOutput() {
  return {
    ...getKindLabelOutput('架空の種別表示名', 'sample-kind'),
    detailHref: screen
      .getByRole('link', { name: /口座全体ノート/ })
      .getAttribute('href'),
  }
}

function getUnsetKindRowOutput() {
  return {
    label: screen.queryByText('種別なし')?.textContent ?? null,
    kindLink: screen.queryByRole('link', {
      name: '種別なし のノートを表示',
    }),
  }
}

describe('NotesList', () => {
  it('ノートを一覧表示し、詳細へのリンクを生成する', async () => {
    const note = makeNote({
      id: 'note-1',
      title: '口座全体ノート',
    })
    const expected = ['口座全体ノート', '/notes/note-1']
    await renderInRouter([note])

    await waitFor(() => {
      expect(getNoteLinkSnapshot()).toEqual(expected)
    })
  })

  it('ノートに設定されたタグを一覧表示する', async () => {
    const note = makeNote({ tags: ['架空タグ', '仮の分類'] })
    await renderInRouter([note])

    expect(
      ['架空タグ', '仮の分類'].map((tag) => screen.getByText(tag).textContent),
    ).toEqual(['架空タグ', '仮の分類'])
  })

  it('displays the catalog name instead of the key in the linked note row', async () => {
    const note = makeNote({
      id: 'note-1',
      title: '口座全体ノート',
      kind: 'sample-kind',
    })
    await renderInRouter([note])

    expect(getLinkedKindRowOutput()).toEqual({
      kindLabel: '架空の種別表示名',
      rawKeyVisible: false,
      detailHref: '/notes/note-1',
    })
  })

  it.each([
    {
      state: 'loading' as const,
      label: '種別を読み込み中',
      noteKinds,
    },
    {
      state: 'error' as const,
      label: '種別を取得できません',
      noteKinds,
    },
    {
      state: 'loaded' as const,
      label: '未登録の種別',
      noteKinds: [],
    },
  ])(
    'does not expose a kind key when catalog state is $state',
    async ({ state, label, noteKinds: availableKinds }) => {
      await renderInRouter([makeNote({ kind: 'unlisted-kind' })], {
        noteKinds: availableKinds,
        noteKindCatalogState: state,
      })

      expect(getKindLabelOutput(label, 'unlisted-kind')).toEqual({
        kindLabel: label,
        rawKeyVisible: false,
      })
    },
  )

  it('shows an unset kind as a non-linked label', async () => {
    await renderInRouter([makeNote({ kind: null })])

    expect(getUnsetKindRowOutput()).toEqual({
      label: '種別なし',
      kindLink: null,
    })
  })

  it('ノートが無ければ空状態を表示する', async () => {
    await renderInRouter([])

    await waitFor(() => {
      expect(screen.getByText('—')).toBeInTheDocument()
    })
  })
})
