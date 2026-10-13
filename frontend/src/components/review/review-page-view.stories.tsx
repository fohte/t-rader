import type { Meta, StoryObj } from '@storybook/react-vite'
import { RouterProvider } from '@tanstack/react-router'
import type { ComponentType } from 'react'

import {
  ReviewPageView,
  type ReviewPageViewProps,
} from '#components/review/review-page-view'
import type { ReviewQueueData } from '#components/review/review-queue-model'
import { createStoryRouter } from '#storybook/story-router'

const data: ReviewQueueData = {
  counts: { notes: 2, annotations: 1, total: 3 },
  items: [
    {
      kind: 'note',
      id: 'fake-version-a',
      noteId: 'fake-note-a',
      versionNo: 3,
      title: '架空企業の決算前メモ',
      createdAt: '2026-10-12T03:00:00Z',
      noteKind: '架空の種別',
      tags: [],
      changeReason: '新しい確認事項を追記しました。',
      createdByKind: 'llm',
      unreadVersionCount: 2,
    },
    {
      kind: 'annotation',
      id: 'fake-annotation-a',
      title: '価格帯を確認する',
      createdAt: '2026-10-11T03:00:00Z',
      targetKind: 'stock',
      targetSymbol: 'DEMO-A',
      timestamp: '2026-10-11T03:00:00Z',
      price: 1234,
    },
    {
      kind: 'note',
      id: 'fake-version-b',
      noteId: 'fake-note-b',
      versionNo: 1,
      title: '架空の観察メモ',
      createdAt: '2026-10-10T03:00:00Z',
      noteKind: null,
      tags: [],
      changeReason: null,
      createdByKind: 'human',
      unreadVersionCount: 1,
    },
  ],
}

function withRouter(Story: ComponentType) {
  return (
    <RouterProvider
      router={createStoryRouter(
        () => (
          <Story />
        ),
        { paths: ['/notes/$noteId', '/annotations/$annoId'] },
      )}
    />
  )
}

const args: ReviewPageViewProps = {
  data,
  navigationSnapshot: null,
  isPending: false,
  isError: false,
  typeFilter: 'all',
  noteKindFilter: 'all',
  selectedItemIds: ['note:fake-version-a', 'note:fake-version-b'],
  isApproving: false,
  hasApprovalError: false,
  onTypeFilterChange: () => {},
  onNoteKindFilterChange: () => {},
  onToggleItem: () => {},
  onToggleVisibleItems: () => {},
  onApproveSelected: () => {},
}

const meta = {
  title: 'Review/ReviewPageView',
  component: ReviewPageView,
  parameters: { layout: 'padded' },
  args,
  decorators: [
    (Story) => (
      <div className="mx-auto max-w-5xl bg-background p-4 text-foreground">
        {withRouter(Story)}
      </div>
    ),
  ],
} satisfies Meta<typeof ReviewPageView>

export default meta
type Story = StoryObj<typeof meta>

export const Populated: Story = {
  name: 'shows mixed unread notes and annotations with selected items.',
}

export const Loading: Story = {
  name: 'shows placeholders while review items load.',
  args: { data: null, isPending: true, selectedItemIds: [] },
}

export const Empty: Story = {
  name: 'shows an empty state when no proposal needs review.',
  args: {
    data: { counts: { notes: 0, annotations: 0, total: 0 }, items: [] },
    selectedItemIds: [],
  },
}

export const Error: Story = {
  name: 'shows an error when review items cannot be loaded.',
  args: { data: null, isError: true, selectedItemIds: [] },
}
