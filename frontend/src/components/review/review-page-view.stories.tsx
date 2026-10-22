import type { Meta, StoryObj } from '@storybook/react-vite'
import { RouterProvider } from '@tanstack/react-router'
import type { ComponentType } from 'react'

import {
  ReviewPageView,
  type ReviewPageViewProps,
} from '#components/review/review-page-view'
import {
  type ReviewQueueData,
  reviewQueueModel,
} from '#components/review/review-queue-model'
import type { components } from '#lib/api/schema.gen'
import { createStoryRouter } from '#storybook/story-router'

type NoteKind = components['schemas']['NoteKind']

const noteKinds: NoteKind[] = [
  {
    key: 'fabricated-kind-key',
    display_name: '架空の種別表示名',
    description: null,
    requires_approval: false,
    sort_order: 1,
  },
]

const data: ReviewQueueData = {
  counts: { notes: 2, annotations: 1, total: 3 },
  items: [
    {
      kind: 'note',
      id: 'fake-version-a',
      noteId: 'fake-note-a',
      versionNo: 3,
      title: '架空企業の決算前メモ',
      createdAt: '2026-01-15T08:00:00+09:00',
      noteKind: 'fabricated-kind-key',
      createdByKind: 'llm',
      unreadVersionCount: 2,
    },
    {
      kind: 'annotation',
      id: 'fake-annotation-a',
      title: '価格帯を確認する',
      createdAt: '2026-01-14T08:00:00+09:00',
      targetSymbol: 'DEMO-A',
      timestamp: '2026-01-14T08:00:00+09:00',
      price: 1234,
    },
    {
      kind: 'note',
      id: 'fake-version-b',
      noteId: 'fake-note-b',
      versionNo: 1,
      title: '架空の確認メモ',
      createdAt: '2026-01-13T09:00:00+09:00',
      noteKind: null,
      createdByKind: 'human',
      unreadVersionCount: 1,
    },
  ],
}

const notesOnlyData: ReviewQueueData = {
  counts: { notes: 2, annotations: 0, total: 2 },
  items: data.items.filter((item) => item.kind === 'note'),
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
  noteKinds,
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

export const AnnotationTab: Story = {
  name: 'the annotation tab shows annotations and disables the note-kind filter.',
  args: { typeFilter: 'annotation', selectedItemIds: [] },
}

export const NoteKindFilter: Story = {
  name: 'the note tab shows only notes with the selected kind.',
  args: {
    typeFilter: 'note',
    noteKindFilter: reviewQueueModel.noteKindFilterKey('fabricated-kind-key'),
    selectedItemIds: [],
  },
}

export const NoteKindFilterOpen: Story = {
  name: 'the note-kind filter displays its available options.',
  args: {
    typeFilter: 'note',
    noteKindFilterOpen: true,
    selectedItemIds: [],
  },
}

export const FilteredEmpty: Story = {
  name: 'the annotation tab shows an empty result when no annotations match.',
  args: {
    data: notesOnlyData,
    typeFilter: 'annotation',
    selectedItemIds: [],
  },
}

export const NoSelection: Story = {
  name: 'the approval action is disabled when no item is selected.',
  args: { selectedItemIds: [] },
}

export const Approving: Story = {
  name: 'the approval action shows progress while the request is running.',
  args: { isApproving: true, selectedItemIds: ['note:fake-version-a'] },
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

export const ApprovalError: Story = {
  name: 'shows the items that remain after an approval fails.',
  args: { hasApprovalError: true },
}
