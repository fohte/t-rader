import { cleanup, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { NoteVersionReviewNavigationView } from '#components/note-detail/note-version-review-navigation-view'
import type { ReviewQueueItem } from '#components/review/review-queue-model'

const firstItem: ReviewQueueItem = {
  kind: 'note',
  id: 'fake-version-a',
  noteId: 'fake-note-a',
  versionNo: 2,
  title: '架空データの調査メモ',
  createdAt: '2026-01-12T02:00:00Z',
  noteKind: '架空の種別',
  createdByKind: 'llm',
  unreadVersionCount: 1,
}

const nextItem: ReviewQueueItem = {
  kind: 'annotation',
  id: 'fake-annotation-a',
  title: '架空データの注記',
  createdAt: '2026-01-12T01:00:00Z',
  targetSymbol: 'demo-code',
  timestamp: '2026-01-12T01:00:00Z',
  price: null,
}

afterEach(() => {
  cleanup()
})

function getNavigationOutput(container: HTMLElement) {
  const navigation = container.querySelector('nav')
  if (navigation == null) return { message: null, actions: [] }

  return {
    message: navigation.querySelector('p')?.textContent.trim() ?? null,
    actions: Array.from(navigation.querySelectorAll('button')).map((button) =>
      button.textContent.trim(),
    ),
  }
}

describe('NoteVersionReviewNavigationView', () => {
  it('shows only a jump to the queue start for an item outside the opened order', () => {
    const { container } = render(
      <NoteVersionReviewNavigationView
        state={{ kind: 'start', total: 2, firstItem }}
        onNavigateToItem={() => {}}
        onReturnToReview={() => {}}
      />,
    )

    expect(getNavigationOutput(container)).toEqual({
      message: '未読 2 件',
      actions: ['未読の先頭へ →'],
    })
  })

  it('returns to review when no unread items remain', () => {
    const { container } = render(
      <NoteVersionReviewNavigationView
        state={{ kind: 'empty', total: 0 }}
        onNavigateToItem={() => {}}
        onReturnToReview={() => {}}
      />,
    )

    expect(getNavigationOutput(container)).toEqual({
      message: '未読はありません',
      actions: ['レビューへ'],
    })
  })

  it('does not offer a next action when the current note is the only unread item', () => {
    const { container } = render(
      <NoteVersionReviewNavigationView
        state={{ kind: 'only-current', total: 1 }}
        onNavigateToItem={() => {}}
        onReturnToReview={() => {}}
      />,
    )

    expect(getNavigationOutput(container)).toEqual({
      message: '未読 1 件 · この項目以外に未読はありません',
      actions: ['レビューへ'],
    })
  })

  it('shows the previous action only when a previous unread item exists', () => {
    const { container } = render(
      <NoteVersionReviewNavigationView
        state={{
          kind: 'item',
          total: 2,
          previousItem: null,
          nextItem,
        }}
        onNavigateToItem={() => {}}
        onReturnToReview={() => {}}
      />,
    )

    expect(getNavigationOutput(container)).toEqual({
      message: '未読 2 件',
      actions: ['次の未読へ →'],
    })
  })

  it('shows previous and next actions for an item between unread items', () => {
    const { container } = render(
      <NoteVersionReviewNavigationView
        state={{
          kind: 'item',
          total: 3,
          previousItem: firstItem,
          nextItem,
        }}
        onNavigateToItem={() => {}}
        onReturnToReview={() => {}}
      />,
    )

    expect(getNavigationOutput(container)).toEqual({
      message: '未読 3 件',
      actions: ['← 前', '次の未読へ →'],
    })
  })

  it('navigates to the next unread item when its action is selected', async () => {
    const user = userEvent.setup()
    const onNavigateToItem = vi.fn()
    render(
      <NoteVersionReviewNavigationView
        state={{
          kind: 'item',
          total: 2,
          previousItem: firstItem,
          nextItem,
        }}
        onNavigateToItem={onNavigateToItem}
        onReturnToReview={() => {}}
      />,
    )

    await user.click(screen.getByRole('button', { name: '次の未読へ →' }))

    expect(onNavigateToItem.mock.calls).toEqual([[nextItem]])
  })

  it('provides a return action when the unread queue cannot load', () => {
    const { container } = render(
      <NoteVersionReviewNavigationView
        state={{ kind: 'error' }}
        onNavigateToItem={() => {}}
        onReturnToReview={() => {}}
      />,
    )

    expect(getNavigationOutput(container)).toEqual({
      message: '未読を読み込めませんでした',
      actions: ['レビューへ'],
    })
  })

  it('shows a loading message while unread items are loading', () => {
    const { container } = render(
      <NoteVersionReviewNavigationView
        state={{ kind: 'loading' }}
        onNavigateToItem={() => {}}
        onReturnToReview={() => {}}
      />,
    )

    expect(getNavigationOutput(container)).toEqual({
      message: '未読を読み込み中',
      actions: [],
    })
  })
})
