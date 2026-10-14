import { RouterProvider } from '@tanstack/react-router'
import { cleanup, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { StockPageActivity } from '#components/stock-page/stock-page-activity'
import type { components } from '#lib/api/schema.gen'
import { createStoryRouter } from '#storybook/story-router'

type Annotation = components['schemas']['Annotation']
type Note = components['schemas']['Note']

afterEach(cleanup)

const annotations: Annotation[] = [
  {
    id: 'demo-annotation',
    target_kind: 'stock',
    target_symbol: 'demo-code',
    timestamp: '2030-04-02T06:00:00Z',
    timestamp_start: null,
    price: null,
    text: '選択処理を確認するデモ用アノテーションです。',
    linked_note_id: null,
    status: 'unread',
    created_by_kind: 'llm',
    execution_step_id: null,
    execution_task_id: null,
    created_at: '2030-04-02T06:00:00Z',
    updated_at: '2030-04-02T06:00:00Z',
  },
]

const notes: Note[] = [
  {
    body_md: '架空コードへの言及を含むテスト用ノートです。',
    created_at: '2030-04-02T06:00:00Z',
    created_by_kind: 'llm',
    frontmatter_json: {},
    graphs_json: [],
    id: 'demo-note',
    is_current: true,
    resolved_price_references_json: {},
    status: 'unread',
    tags: [],
    title: '銘柄を確認したデモ用ノート',
    updated_at: '2030-04-02T06:00:00Z',
    version_id: 'demo-note-version',
    version_no: 1,
  },
]

describe('StockPageActivity', () => {
  it('selects an annotation when its row is clicked', async () => {
    const onSelectAnnotation = vi.fn()
    const user = userEvent.setup()
    const router = createStoryRouter(
      () => (
        <StockPageActivity
          annotations={annotations}
          notes={notes}
          trades={[]}
          activeTab="annotations"
          onTabChange={() => undefined}
          selectedAnnotationId={null}
          onSelectAnnotation={onSelectAnnotation}
        />
      ),
      { paths: ['/annotations/$annoId', '/notes/$noteId'] },
    )

    render(<RouterProvider router={router} />)
    await user.click(
      await screen.findByRole('button', { name: /選択処理を確認する/ }),
    )

    expect(onSelectAnnotation.mock.calls).toEqual([['demo-annotation']])
  })

  it('switches to the trade tab when it is clicked', async () => {
    const onTabChange = vi.fn()
    const user = userEvent.setup()
    const router = createStoryRouter(
      () => (
        <StockPageActivity
          annotations={annotations}
          notes={notes}
          trades={[]}
          activeTab="annotations"
          onTabChange={onTabChange}
          selectedAnnotationId={null}
          onSelectAnnotation={() => undefined}
        />
      ),
      { paths: ['/annotations/$annoId', '/notes/$noteId'] },
    )

    render(<RouterProvider router={router} />)
    await user.click(await screen.findByRole('tab', { name: /取引/ }))

    expect(onTabChange.mock.calls).toEqual([['trades']])
  })

  it('switches to the notes tab when it is clicked', async () => {
    const onTabChange = vi.fn()
    const user = userEvent.setup()
    const router = createStoryRouter(
      () => (
        <StockPageActivity
          annotations={annotations}
          notes={notes}
          trades={[]}
          activeTab="annotations"
          onTabChange={onTabChange}
          selectedAnnotationId={null}
          onSelectAnnotation={() => undefined}
        />
      ),
      { paths: ['/annotations/$annoId', '/notes/$noteId'] },
    )

    render(<RouterProvider router={router} />)
    await user.click(await screen.findByRole('tab', { name: /ノート/ }))

    expect(onTabChange.mock.calls).toEqual([['notes']])
  })

  it('links an unread note to its detail page with its author and update date', async () => {
    const router = createStoryRouter(
      () => (
        <StockPageActivity
          annotations={[]}
          notes={notes}
          trades={[]}
          activeTab="notes"
          onTabChange={() => undefined}
          selectedAnnotationId={null}
          onSelectAnnotation={() => undefined}
        />
      ),
      { paths: ['/notes/$noteId'] },
    )

    render(<RouterProvider router={router} />)
    await screen.findByRole('link', {
      name: /銘柄を確認したデモ用ノート/,
    })

    expect(
      screen
        .getAllByRole('link')
        .map((link) => [
          link.getAttribute('href'),
          link.textContent.includes('エージェント'),
          link.textContent.includes('未レビュー'),
          link.querySelector('time')?.getAttribute('dateTime'),
        ]),
    ).toEqual([['/notes/demo-note', true, true, '2030-04-02T06:00:00Z']])
  })
})
