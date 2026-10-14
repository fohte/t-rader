import type { Meta, StoryObj } from '@storybook/react-vite'
import { RouterProvider } from '@tanstack/react-router'
import type { ComponentType } from 'react'

import { ReviewNavigationBand } from '#components/review/review-navigation-band'
import {
  type ReviewQueueItem,
  reviewQueueModel,
} from '#components/review/review-queue-model'
import { createStoryRouter } from '#storybook/story-router'

const items: ReviewQueueItem[] = [
  {
    kind: 'annotation',
    id: 'sample-annotation-01',
    title: '値動きの変化を記録した注釈',
    createdAt: '2026-01-14T03:00:00Z',
    targetSymbol: 'sample-code',
    timestamp: '2026-01-14T02:30:00Z',
    price: null,
  },
  {
    kind: 'note',
    id: 'sample-version-02',
    noteId: 'sample-note-02',
    versionNo: 3,
    title: '定期確認の記録',
    createdAt: '2026-01-14T02:00:00Z',
    noteKind: null,
    createdByKind: 'llm',
    unreadVersionCount: 1,
  },
  {
    kind: 'annotation',
    id: 'sample-annotation-03',
    title: '取引量の変化を記録した注釈',
    createdAt: '2026-01-14T01:00:00Z',
    targetSymbol: 'sample-code',
    timestamp: '2026-01-14T00:30:00Z',
    price: 1200,
  },
]

const snapshot = reviewQueueModel.createNavigationSnapshot(items)
const lastUnreadItems = items.slice(0, 2)
const lastUnreadSnapshot =
  reviewQueueModel.createNavigationSnapshot(lastUnreadItems)
const onlyCurrentItems = items.slice(0, 1)
const onlyCurrentSnapshot =
  reviewQueueModel.createNavigationSnapshot(onlyCurrentItems)

function withRouter(Story: ComponentType) {
  return (
    <RouterProvider
      router={createStoryRouter(
        () => (
          <Story />
        ),
        {
          paths: ['/review', '/notes/$noteId', '/annotations/$annoId'],
        },
      )}
    />
  )
}

const meta = {
  title: 'Review/ReviewNavigationBand',
  component: ReviewNavigationBand,
  decorators: [withRouter],
  args: {
    items,
    snapshot,
    currentItemKey: 'note:sample-version-02',
  },
} satisfies Meta<typeof ReviewNavigationBand>

export default meta
type Story = StoryObj<typeof meta>

export const BetweenUnreadItems: Story = {
  name: 'shows previous and next unread items in the queue.',
  args: { items, snapshot, currentItemKey: 'note:sample-version-02' },
}

export const FirstUnreadItem: Story = {
  name: 'hides the previous link for the first unread item.',
  args: {
    items,
    snapshot,
    currentItemKey: 'annotation:sample-annotation-01',
  },
}

export const LastUnreadItem: Story = {
  name: 'wraps the next link from the last unread item to the first.',
  args: {
    items: lastUnreadItems,
    snapshot: lastUnreadSnapshot,
    currentItemKey: 'note:sample-version-02',
  },
}

export const ItemOutsideQueue: Story = {
  name: 'shows only the unread count and first item outside the queue.',
  args: {
    items,
    snapshot,
    currentItemKey: 'annotation:sample-annotation-outside',
  },
}

export const NoUnreadItems: Story = {
  name: 'links to the review page when no unread items remain.',
  args: {
    items: [],
    snapshot,
    currentItemKey: 'annotation:sample-annotation-01',
  },
}

export const OnlyCurrentUnreadItem: Story = {
  name: 'hides navigation links when the current item is the only unread item.',
  args: {
    items: onlyCurrentItems,
    snapshot: onlyCurrentSnapshot,
    currentItemKey: 'annotation:sample-annotation-01',
  },
}

export const Loading: Story = {
  name: 'shows the loading state while unread navigation is pending.',
  args: { isPending: true },
}
