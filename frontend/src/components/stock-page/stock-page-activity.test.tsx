import { RouterProvider } from '@tanstack/react-router'
import { cleanup, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { StockPageActivity } from '#components/stock-page/stock-page-activity'
import type { components } from '#lib/api/schema.gen'
import { createStoryRouter } from '#storybook/story-router'

type Annotation = components['schemas']['Annotation']

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

describe('StockPageActivity', () => {
  it('selects an annotation when its row is clicked', async () => {
    const onSelectAnnotation = vi.fn()
    const user = userEvent.setup()
    const router = createStoryRouter(
      () => (
        <StockPageActivity
          annotations={annotations}
          trades={[]}
          financialSummaries={[]}
          financialSummariesState="ready"
          groups={[]}
          groupsState="ready"
          activeTab="annotations"
          onTabChange={() => undefined}
          selectedAnnotationId={null}
          onSelectAnnotation={onSelectAnnotation}
        />
      ),
      { paths: ['/annotations/$annoId'] },
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
          trades={[]}
          financialSummaries={[]}
          financialSummariesState="ready"
          groups={[]}
          groupsState="ready"
          activeTab="annotations"
          onTabChange={onTabChange}
          selectedAnnotationId={null}
          onSelectAnnotation={() => undefined}
        />
      ),
      { paths: ['/annotations/$annoId'] },
    )

    render(<RouterProvider router={router} />)
    await user.click(await screen.findByRole('tab', { name: /取引/ }))

    expect(onTabChange.mock.calls).toEqual([['trades']])
  })

  it('switches to the financial tab when it is clicked', async () => {
    const onTabChange = vi.fn()
    const user = userEvent.setup()
    const router = createStoryRouter(
      () => (
        <StockPageActivity
          annotations={annotations}
          trades={[]}
          financialSummaries={[]}
          financialSummariesState="ready"
          groups={[]}
          groupsState="ready"
          activeTab="annotations"
          onTabChange={onTabChange}
          selectedAnnotationId={null}
          onSelectAnnotation={() => undefined}
        />
      ),
      { paths: ['/annotations/$annoId'] },
    )

    render(<RouterProvider router={router} />)
    await user.click(await screen.findByRole('tab', { name: '財務' }))

    expect(onTabChange.mock.calls).toEqual([['financials']])
  })
})
