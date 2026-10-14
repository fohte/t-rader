import { RouterProvider } from '@tanstack/react-router'
import { cleanup, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { useState } from 'react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import {
  type ReviewQueueData,
  type ReviewQueueItem,
  reviewQueueModel,
} from '#components/review/review-queue-model'
import { useReviewNavigation } from '#components/review/use-review-navigation'
import { useReviewQueue } from '#components/review/use-review-queue'
import { createStoryRouter } from '#storybook/story-router'

vi.mock('#components/review/use-review-queue', () => ({
  useReviewQueue: vi.fn(),
}))

type ReviewQueueState = ReturnType<typeof useReviewQueue>

const currentItemKey = 'annotation:sample-annotation-01'
const item: ReviewQueueItem = {
  kind: 'annotation',
  id: 'sample-annotation-01',
  title: 'サンプルの注釈',
  createdAt: '2026-01-14T03:00:00Z',
  targetSymbol: 'sample-code',
  timestamp: '2026-01-14T02:30:00Z',
  price: null,
}
const queueData: ReviewQueueData = {
  counts: { notes: 0, annotations: 1, total: 1 },
  items: [item],
}
const invalidate = vi.fn()
const pendingQueue: ReviewQueueState = {
  data: null,
  isPending: true,
  isError: false,
  invalidate,
}
const loadedQueue: ReviewQueueState = {
  data: queueData,
  isPending: false,
  isError: false,
  invalidate,
}

afterEach(cleanup)

function ReviewNavigationProbe() {
  const [queue, setQueue] = useState(pendingQueue)
  vi.mocked(useReviewQueue).mockReturnValue(queue)
  const { navigation, isPending, isError } = useReviewNavigation(currentItemKey)

  return (
    <>
      <output data-testid="review-navigation-state">
        {JSON.stringify({ navigation, isPending, isError })}
      </output>
      <button
        type="button"
        onClick={() => {
          setQueue(loadedQueue)
        }}
      >
        Load queue
      </button>
    </>
  )
}

describe('useReviewNavigation', () => {
  it('creates a fallback snapshot after a direct-open queue finishes loading', async () => {
    const user = userEvent.setup()
    const router = createStoryRouter(() => <ReviewNavigationProbe />, {
      paths: ['/annotations/$annoId'],
      initialPath: '/annotations/sample-annotation-01',
    })
    render(<RouterProvider router={router} />)

    const output = await screen.findByTestId('review-navigation-state')
    expect(output.textContent).toEqual(
      JSON.stringify({ navigation: null, isPending: true, isError: false }),
    )

    await user.click(screen.getByRole('button', { name: 'Load queue' }))

    const expectedNavigation = {
      items: [item],
      snapshot: reviewQueueModel.createNavigationSnapshot([item]),
      currentItemKey,
    }
    const expected = JSON.stringify({
      navigation: expectedNavigation,
      isPending: false,
      isError: false,
    })

    expect((await screen.findByText(expected)).textContent).toEqual(expected)
  })
})
