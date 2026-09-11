import {
  createMemoryHistory,
  createRootRoute,
  createRoute,
  createRouter,
  RouterProvider,
} from '@tanstack/react-router'
import { cleanup, render, screen, waitFor } from '@testing-library/react'
import { afterEach, describe, expect, it } from 'vitest'

import { AnnotationList } from '#components/strategy-home/annotation-list'
import type { NumberedAnnotation } from '#lib/annotation-utils'

function makeAnnotation(
  overrides: Partial<NumberedAnnotation> = {},
): NumberedAnnotation {
  return {
    id: overrides.id ?? crypto.randomUUID(),
    label: overrides.label ?? 'A1',
    strategy_id: overrides.strategy_id ?? null,
    target_kind: overrides.target_kind ?? 'stock',
    target_symbol: overrides.target_symbol ?? '7203',
    text: overrides.text ?? 'text',
    status: overrides.status ?? 'unread',
    linked_note_id: overrides.linked_note_id ?? null,
    price: overrides.price ?? null,
    created_by_kind: overrides.created_by_kind ?? 'llm',
    timestamp: overrides.timestamp ?? '2026-01-01T00:00:00Z',
    created_at: overrides.created_at ?? '2026-01-01T00:00:00Z',
    updated_at: overrides.updated_at ?? '2026-01-01T00:00:00Z',
  }
}

afterEach(cleanup)

// Link が親ルートを要求するため、最低限のテストルーターを噛ませる
async function renderInRouter(items: NumberedAnnotation[]) {
  const rootRoute = createRootRoute({
    component: () => <AnnotationList items={items} />,
  })
  const annoRoute = createRoute({
    getParentRoute: () => rootRoute,
    path: '/annotations/$annoId',
    component: () => null,
  })
  const noteRoute = createRoute({
    getParentRoute: () => rootRoute,
    path: '/notes/$noteId',
    component: () => null,
  })
  const router = createRouter({
    routeTree: rootRoute.addChildren([annoRoute, noteRoute]),
    history: createMemoryHistory({ initialEntries: ['/'] }),
  })
  render(<RouterProvider router={router} />)
  await waitFor(() => {
    expect(
      document.body.firstElementChild?.children.length ?? 0,
    ).toBeGreaterThan(0)
  })
}

describe('AnnotationList', () => {
  it('strategy_id が null のアノテーションも表示し、/annotations/$annoId へのリンクを生成する', async () => {
    const anno = makeAnnotation({
      id: 'anno-1',
      strategy_id: null,
      text: '口座全体アノテーション',
      linked_note_id: 'note-1',
    })
    await renderInRouter([anno])

    await waitFor(() => {
      expect(screen.getByText('口座全体アノテーション')).toBeInTheDocument()
    })
    expect(screen.getByRole('link', { name: '→ 詳細' })).toHaveAttribute(
      'href',
      '/annotations/anno-1',
    )
    expect(screen.getByRole('link', { name: '→ note を開く' })).toHaveAttribute(
      'href',
      '/notes/note-1',
    )
  })

  it('アノテーションが無ければ何も描画しない', () => {
    render(<AnnotationList items={[]} />)
    expect(document.body.textContent).toBe('')
  })
})
