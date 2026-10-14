import { RouterProvider } from '@tanstack/react-router'
import { cleanup, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it } from 'vitest'

import { ReviewNavigationBand } from '#components/review/review-navigation-band'
import {
  type ReviewNavigationSnapshot,
  type ReviewQueueItem,
  reviewQueueModel,
} from '#components/review/review-queue-model'
import { createStoryRouter } from '#storybook/story-router'

const items: ReviewQueueItem[] = [
  {
    kind: 'annotation',
    id: 'sample-annotation-01',
    title: '最初の注釈',
    createdAt: '2026-10-15T03:00:00Z',
    targetSymbol: 'sample-code',
    timestamp: '2026-10-15T02:30:00Z',
    price: null,
  },
  {
    kind: 'note',
    id: 'sample-version-02',
    noteId: 'sample-note-02',
    versionNo: 3,
    title: '確認用のメモ',
    createdAt: '2026-10-15T02:00:00Z',
    noteKind: null,
    createdByKind: 'llm',
    unreadVersionCount: 1,
  },
  {
    kind: 'annotation',
    id: 'sample-annotation-03',
    title: '最後の注釈',
    createdAt: '2026-10-15T01:00:00Z',
    targetSymbol: 'sample-code',
    timestamp: '2026-10-15T00:30:00Z',
    price: 1200,
  },
]

const snapshot = reviewQueueModel.createNavigationSnapshot(items)

afterEach(cleanup)

function renderBand({
  items: queueItems,
  snapshot: navigationSnapshot,
  currentItemKey,
}: {
  items: ReviewQueueItem[]
  snapshot: ReviewNavigationSnapshot
  currentItemKey: string
}) {
  const router = createStoryRouter(
    () => (
      <ReviewNavigationBand
        items={queueItems}
        snapshot={navigationSnapshot}
        currentItemKey={currentItemKey}
      />
    ),
    {
      paths: ['/review', '/notes/$noteId', '/annotations/$annoId'],
      initialPath: '/annotations/sample-annotation-02',
    },
  )
  const rendered = render(<RouterProvider router={router} />)
  return { ...rendered, router }
}

function getRenderedOutput(container: HTMLElement) {
  return {
    text: container.textContent,
    links: [...container.querySelectorAll('a')].map((link) => ({
      label: link.textContent,
      href: link.getAttribute('href'),
    })),
  }
}

function getRenderedStatus(container: HTMLElement) {
  return {
    text: container.textContent,
    busy: container.querySelector('[role="status"]')?.getAttribute('aria-busy'),
  }
}

describe('ReviewNavigationBand', () => {
  it.each([
    {
      name: 'shows previous and next links for an item between unread items',
      items,
      currentItemKey: 'note:sample-version-02',
      expected: {
        text: 'レビュー · 残り 3 件 ← 前の未読次の未読へ →',
        links: [
          {
            label: '← 前の未読',
            href: '/annotations/sample-annotation-01',
          },
          { label: '次の未読へ →', href: '/annotations/sample-annotation-03' },
        ],
      },
    },
    {
      name: 'hides the previous link at the start of the queue',
      items,
      currentItemKey: 'annotation:sample-annotation-01',
      expected: {
        text: 'レビュー · 残り 3 件 次の未読へ →',
        links: [
          { label: '次の未読へ →', href: '/notes/sample-note-02?version=3' },
        ],
      },
    },
    {
      name: 'wraps the next link to the first item at the end of the queue',
      items,
      currentItemKey: 'annotation:sample-annotation-03',
      expected: {
        text: 'レビュー · 残り 3 件 ← 前の未読次の未読へ →',
        links: [
          { label: '← 前の未読', href: '/notes/sample-note-02?version=3' },
          { label: '次の未読へ →', href: '/annotations/sample-annotation-01' },
        ],
      },
    },
    {
      name: 'shows only the count and first link for an item outside the snapshot',
      items,
      currentItemKey: 'annotation:sample-annotation-outside',
      expected: {
        text: '未読 3 件 · 未読の先頭へ →',
        links: [
          {
            label: '未読の先頭へ →',
            href: '/annotations/sample-annotation-01',
          },
        ],
      },
    },
    {
      name: 'links to the review page when the queue is empty',
      items: [],
      currentItemKey: 'annotation:sample-annotation-01',
      expected: {
        text: '未読はありません · レビューへ',
        links: [{ label: 'レビューへ', href: '/review' }],
      },
    },
    {
      name: 'hides the next link when the current item is the only unread item',
      items: items.slice(0, 1),
      currentItemKey: 'annotation:sample-annotation-01',
      expected: {
        text: 'レビュー · 残り 1 件',
        links: [],
      },
    },
  ])('$name', async ({ items: queueItems, currentItemKey, expected }) => {
    const { container } = renderBand({
      items: queueItems,
      snapshot,
      currentItemKey,
    })
    if (expected.links.length > 0) {
      await screen.findAllByRole('link')
    } else {
      await screen.findByText(expected.text)
    }

    expect(getRenderedOutput(container)).toEqual(expected)
  })

  it('shows a busy state while the unread queue is loading', async () => {
    const router = createStoryRouter(() => <ReviewNavigationBand isPending />, {
      paths: ['/review', '/notes/$noteId', '/annotations/$annoId'],
    })
    const { container } = render(<RouterProvider router={router} />)
    await screen.findByRole('status')

    expect(getRenderedStatus(container)).toEqual({
      text: '未読を読み込んでいます',
      busy: 'true',
    })
  })

  it('keeps the original snapshot and changes the current key when following a note', async () => {
    const user = userEvent.setup()
    const { router } = renderBand({
      items,
      snapshot,
      currentItemKey: 'annotation:sample-annotation-01',
    })

    await user.click(await screen.findByRole('link', { name: '次の未読へ →' }))

    expect(
      reviewQueueModel.readNavigationState(router.state.location.state),
    ).toEqual({
      snapshot,
      currentItemKey: 'note:sample-version-02',
    })
  })
})
