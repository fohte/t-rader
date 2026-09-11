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

function makeNote(overrides: Partial<Note> = {}): Note {
  return {
    id: overrides.id ?? crypto.randomUUID(),
    strategy_id: overrides.strategy_id ?? null,
    title: overrides.title ?? 'title',
    body_md: overrides.body_md ?? 'body',
    frontmatter_json: overrides.frontmatter_json ?? {},
    graphs_json: overrides.graphs_json ?? [],
    type_tag: overrides.type_tag ?? null,
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
async function renderInRouter(notes: Note[]) {
  const rootRoute = createRootRoute({
    component: () => <NotesList notes={notes} />,
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

describe('NotesList', () => {
  it('strategy_id が null のノートも一覧表示し、/notes/$noteId へのリンクを生成する', async () => {
    const note = makeNote({
      id: 'note-1',
      strategy_id: null,
      title: '口座全体ノート',
    })
    await renderInRouter([note])

    await waitFor(() => {
      expect(screen.getByText('口座全体ノート')).toBeInTheDocument()
    })
    expect(
      screen.getByRole('link', { name: /口座全体ノート/ }),
    ).toHaveAttribute('href', '/notes/note-1')
  })

  it('ノートが無ければ空状態を表示する', async () => {
    await renderInRouter([])

    await waitFor(() => {
      expect(screen.getByText('—')).toBeInTheDocument()
    })
  })
})
