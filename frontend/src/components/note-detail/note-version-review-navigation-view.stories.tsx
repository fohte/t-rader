import type { Meta, StoryObj } from '@storybook/react-vite'

import { NoteVersionReviewNavigationView } from '#components/note-detail/note-version-review-navigation-view'
import type { ReviewQueueItem } from '#components/review/review-queue-model'

const previousNote: ReviewQueueItem = {
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

const nextAnnotation: ReviewQueueItem = {
  kind: 'annotation',
  id: 'fake-annotation-a',
  title: '架空データの注記',
  createdAt: '2026-01-12T01:00:00Z',
  targetSymbol: 'demo-code',
  timestamp: '2026-01-12T01:00:00Z',
  price: null,
}

const meta = {
  title: 'NoteDetail/NoteVersionReviewNavigationView',
  component: NoteVersionReviewNavigationView,
  parameters: { layout: 'padded' },
  args: {
    onNavigateToItem: () => {},
    onReturnToReview: () => {},
  },
  decorators: [
    (Story) => (
      <div className="max-w-4xl bg-background text-foreground">
        <Story />
      </div>
    ),
  ],
} satisfies Meta<typeof NoteVersionReviewNavigationView>

export default meta
type Story = StoryObj<typeof meta>

export const CurrentItem: Story = {
  name: 'shows previous and next unread actions for an item in the queue.',
  args: {
    state: {
      kind: 'item',
      total: 3,
      previousItem: previousNote,
      nextItem: nextAnnotation,
    },
  },
}

export const FirstItem: Story = {
  name: 'hides the previous action for the first item in the queue.',
  args: {
    state: {
      kind: 'item',
      total: 3,
      previousItem: null,
      nextItem: nextAnnotation,
    },
  },
}

export const QueueStart: Story = {
  name: 'shows only the first unread action for an item outside the queue.',
  args: {
    state: {
      kind: 'start',
      total: 2,
      firstItem: nextAnnotation,
    },
  },
}

export const QueueEmpty: Story = {
  name: 'sends the user to review when no unread items remain.',
  args: { state: { kind: 'empty', total: 0 } },
}

export const OnlyCurrentItem: Story = {
  name: 'shows when the current note is the only unread item.',
  args: { state: { kind: 'only-current', total: 1 } },
}

export const QueueLoading: Story = {
  name: 'shows a loading message while unread items are loading.',
  args: { state: { kind: 'loading' } },
}

export const QueueError: Story = {
  name: 'offers a path back to review when unread items cannot load.',
  args: { state: { kind: 'error' } },
}
