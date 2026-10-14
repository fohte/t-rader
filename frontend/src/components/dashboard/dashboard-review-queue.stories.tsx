import type { Meta, StoryObj } from '@storybook/react-vite'
import { RouterProvider } from '@tanstack/react-router'
import type { ComponentType } from 'react'

import { DashboardReviewQueue } from '#components/dashboard/dashboard-review-queue'
import type { ReviewQueueData } from '#components/review/review-queue-model'
import { createStoryRouter } from '#storybook/story-router'

const data: ReviewQueueData = {
  counts: { notes: 1, annotations: 1, total: 2 },
  items: [
    {
      kind: 'note',
      id: 'fake-version-a',
      noteId: 'fake-note-a',
      versionNo: 2,
      title: '架空企業の決算前メモ',
      createdAt: '2026-10-10T03:00:00Z',
      noteKind: '架空の種別',
      createdByKind: 'llm',
      unreadVersionCount: 1,
    },
    {
      kind: 'annotation',
      id: 'fake-annotation-a',
      title: '出来高を確認する価格帯',
      createdAt: '2026-10-09T03:00:00Z',
      targetSymbol: 'DEMO-A',
      timestamp: '2026-10-09T03:00:00Z',
      price: null,
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
        {
          paths: ['/review', '/notes/$noteId', '/annotations/$annoId'],
        },
      )}
    />
  )
}

const meta = {
  title: 'Dashboard/DashboardReviewQueue',
  component: DashboardReviewQueue,
  decorators: [withRouter],
  args: { data, isPending: false, isError: false, variant: 'panel' },
} satisfies Meta<typeof DashboardReviewQueue>

export default meta
type Story = StoryObj<typeof meta>

export const Panel: Story = {
  name: 'shows review counts and the latest unread items.',
}

export const Compact: Story = {
  name: 'shows the total review count in one compact row.',
  args: { variant: 'compact' },
}

export const Empty: Story = {
  name: 'shows an empty state when there is nothing to review.',
  args: {
    data: {
      counts: { notes: 0, annotations: 0, total: 0 },
      items: [],
    },
  },
}

export const Loading: Story = {
  name: 'shows placeholders while review items are loading.',
  args: { data: null, isPending: true },
}

export const Error: Story = {
  name: 'shows an error message when review items cannot be loaded.',
  args: { data: null, isPending: false, isError: true },
}

export const CompactError: Story = {
  name: 'shows an unavailable count when compact review data cannot be loaded.',
  args: {
    data: null,
    isPending: false,
    isError: true,
    variant: 'compact',
  },
}
