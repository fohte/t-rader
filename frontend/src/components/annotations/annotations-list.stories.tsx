import type { Meta, StoryObj } from '@storybook/react-vite'
import { RouterProvider } from '@tanstack/react-router'
import type { ComponentType } from 'react'

import type { AnnotationsListProps } from '#components/annotations/annotations-list'
import { AnnotationsList } from '#components/annotations/annotations-list'
import type { components } from '#lib/api/schema.gen'
import { createStoryRouter } from '#storybook/story-router'

type Annotation = components['schemas']['Annotation']

const annotations: Annotation[] = [
  {
    id: '00000000-0000-0000-0000-000000000001',
    target_kind: 'stock',
    target_symbol: 'DEMO-A',
    timestamp: '2026-01-02T00:00:00Z',
    price: 1250,
    text: '架空の検証メモです。表示名と本文が長い場合の折り返しを確認します。',
    linked_note_id: null,
    status: 'unread',
    created_by_kind: 'llm',
    execution_step_id: null,
    execution_task_id: null,
    created_at: '2026-01-02T00:00:00Z',
    updated_at: '2026-01-02T00:00:00Z',
  },
  {
    id: '00000000-0000-0000-0000-000000000002',
    target_kind: 'stock',
    target_symbol: 'DEMO-LONG-SYMBOL-NAME',
    timestamp: '2026-01-01T00:00:00Z',
    price: null,
    text: '長い表示名を含む架空の確認用アノテーションです。',
    linked_note_id: null,
    status: 'approved',
    created_by_kind: 'human',
    execution_step_id: null,
    execution_task_id: null,
    created_at: '2026-01-01T00:00:00Z',
    updated_at: '2026-01-01T00:00:00Z',
  },
  {
    id: '00000000-0000-0000-0000-000000000003',
    target_kind: 'stock',
    target_symbol: 'DEMO-B',
    timestamp: '2025-12-31T00:00:00Z',
    price: null,
    text: 'もう 1 件の架空の確認用アノテーションです。',
    linked_note_id: null,
    status: 'unread',
    created_by_kind: 'llm',
    execution_step_id: null,
    execution_task_id: null,
    created_at: '2025-12-31T00:00:00Z',
    updated_at: '2025-12-31T00:00:00Z',
  },
  {
    id: '00000000-0000-0000-0000-000000000004',
    target_kind: 'stock',
    target_symbol: 'DEMO-C',
    timestamp: '2025-12-30T00:00:00Z',
    price: null,
    text: '架空の却下済みアノテーションです。',
    linked_note_id: null,
    status: 'rejected',
    created_by_kind: 'human',
    execution_step_id: null,
    execution_task_id: null,
    created_at: '2025-12-30T00:00:00Z',
    updated_at: '2025-12-30T00:00:00Z',
  },
]

const pendingAnnotations = annotations.filter(
  (annotation) => annotation.status === 'unread',
)

function withRouter(Story: ComponentType) {
  return (
    <RouterProvider
      router={createStoryRouter(
        () => (
          <Story />
        ),
        {
          paths: ['/annotations/$annoId'],
        },
      )}
    />
  )
}

const args: AnnotationsListProps = {
  annotations: pendingAnnotations,
  view: 'pending',
  state: 'loaded',
  selectedItemIds: ['annotation:00000000-0000-0000-0000-000000000001'],
  isApproving: false,
  hasApprovalError: false,
  onViewChange: () => {},
  onToggleItem: () => {},
  onToggleVisibleItems: () => {},
  onApproveSelected: () => {},
}

const meta = {
  title: 'Annotations/AnnotationsList',
  component: AnnotationsList,
  parameters: { layout: 'padded' },
  decorators: [
    (Story) => (
      <div className="mx-auto max-w-5xl bg-background p-4 text-foreground">
        {withRouter(Story)}
      </div>
    ),
  ],
  args,
} satisfies Meta<typeof AnnotationsList>

export default meta
type Story = StoryObj<typeof meta>

export const PendingSelection: Story = {
  name: 'the unread list shows a selected annotation and its approval action.',
}

export const AllStatuses: Story = {
  name: 'the full list distinguishes unread and approved annotations.',
  args: {
    annotations,
    view: 'all',
    selectedItemIds: [],
  },
}

export const MobileSelection: Story = {
  name: 'the selected annotation shows the mobile approval action.',
  globals: { viewport: { value: 'mobileAnnotations' } },
  parameters: {
    viewport: {
      options: {
        mobileAnnotations: {
          name: 'Mobile annotations',
          styles: { width: '390px', height: '700px' },
          type: 'mobile',
        },
      },
    },
    screenshot: { viewport: { width: 390, height: 700 } },
  },
}

export const ApprovalFailed: Story = {
  name: 'the failed annotation and remaining selection stay visible.',
  args: {
    selectedItemIds: [
      'annotation:00000000-0000-0000-0000-000000000001',
      'annotation:00000000-0000-0000-0000-000000000003',
    ],
    hasApprovalError: true,
  },
}

export const Loading: Story = {
  name: 'the annotation list shows loading placeholders.',
  args: { annotations: [], state: 'loading', selectedItemIds: [] },
}

export const Unavailable: Story = {
  name: 'the annotation list shows an error when retrieval fails.',
  args: { annotations: [], state: 'error', selectedItemIds: [] },
}

export const EmptyUnread: Story = {
  name: 'the unread list explains that it has no annotations.',
  args: {
    annotations: [],
    view: 'pending',
    state: 'loaded',
    selectedItemIds: [],
  },
}

export const EmptyAll: Story = {
  name: 'the full list explains that it has no annotations.',
  args: {
    annotations: [],
    view: 'all',
    state: 'loaded',
    selectedItemIds: [],
  },
}
