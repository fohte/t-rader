import type { Meta, StoryObj } from '@storybook/react-vite'
import { RouterProvider } from '@tanstack/react-router'
import { fn } from 'storybook/test'

import { AnnotationDetail } from '#components/annotations/annotation-detail'
import type { components } from '#lib/api/schema.gen'
import { createStoryRouter } from '#storybook/story-router'

type Annotation = components['schemas']['Annotation']
type Comment = components['schemas']['Comment']
type ChangeHistory = components['schemas']['ChangeHistory']

const unreadAnnotation: Annotation = {
  id: '00000000-0000-0000-0000-000000000011',
  target_kind: 'stock',
  target_symbol: 'sample-code',
  timestamp: '2026-10-13T01:23:00Z',
  price: 1234,
  text: 'サンプルの注釈本文です。',
  linked_note_id: null,
  status: 'unread',
  created_by_kind: 'llm',
  execution_step_id: null,
  execution_task_id: null,
  created_at: '2026-10-13T01:24:00Z',
  updated_at: '2026-10-13T01:24:00Z',
}

const approvedAnnotation: Annotation = {
  ...unreadAnnotation,
  id: '00000000-0000-0000-0000-000000000012',
  price: null,
  linked_note_id: '00000000-0000-0000-0000-000000000013',
  status: 'approved',
}

const rejectedAnnotation: Annotation = {
  ...unreadAnnotation,
  id: '00000000-0000-0000-0000-000000000014',
  linked_note_id: null,
  status: 'rejected',
}

const approvedComment: Comment = {
  id: '00000000-0000-0000-0000-000000000015',
  target_id: approvedAnnotation.id,
  target_kind: 'annotation',
  parent_id: null,
  author_kind: 'human',
  author_label: 'サンプル作成者',
  body: 'サンプルのコメントです。',
  resolved: false,
  anchor_side: null,
  anchor_text: null,
  start_line: null,
  end_line: null,
  created_at: '2026-10-13T01:25:00Z',
}

const rejectedComment: Comment = {
  ...approvedComment,
  id: '00000000-0000-0000-0000-000000000017',
  target_id: rejectedAnnotation.id,
}

const historyEntry: ChangeHistory = {
  id: '00000000-0000-0000-0000-000000000016',
  target_id: approvedAnnotation.id,
  target_kind: 'annotation',
  actor_kind: 'human',
  actor_label: 'サンプル作成者',
  op: 'approved',
  summary: '確認済み',
  diff_json: {},
  created_at: '2026-10-13T01:26:00Z',
}

const meta = {
  title: 'Annotations/AnnotationDetail',
  component: AnnotationDetail,
  decorators: [
    (Story) => (
      <RouterProvider
        router={createStoryRouter(
          () => (
            <Story />
          ),
          {
            paths: ['/notes', '/notes/$noteId', '/charts/$instrumentId'],
          },
        )}
      />
    ),
  ],
  args: {
    annotation: unreadAnnotation,
    isPending: false,
    comments: [],
    history: [],
    stockName: 'サンプル銘柄',
    commentDraft: '',
    isApprovalPending: false,
    isRejectionPending: false,
    isCommentPending: false,
    approvalError: null,
    rejectionError: null,
    commentError: null,
    onApprove: fn(),
    onReject: fn(),
    onAddComment: (event) => {
      event.preventDefault()
    },
    onCommentDraftChange: fn(),
  },
} satisfies Meta<typeof AnnotationDetail>

export default meta
type Story = StoryObj<typeof meta>

export const Loading: Story = {
  name: 'shows the loading state while the annotation is pending.',
  args: { isPending: true },
}

export const NotFound: Story = {
  name: 'shows a message when the annotation cannot be found.',
  args: { annotation: undefined },
}

export const UnreadWithoutRelatedItems: Story = {
  name: 'shows an unread annotation without comments or a linked note.',
}

export const ApprovedWithRelatedItems: Story = {
  name: 'shows an approved annotation with comments and a linked note.',
  args: {
    annotation: approvedAnnotation,
    comments: [approvedComment],
    history: [historyEntry],
  },
}

export const RejectedWithComments: Story = {
  name: 'shows a rejected annotation with comments and no linked note.',
  args: {
    annotation: rejectedAnnotation,
    comments: [rejectedComment],
  },
}

export const ApprovalError: Story = {
  name: 'shows a message when the review action fails.',
  args: { approvalError: new Error('sample failure') },
}

export const CommentError: Story = {
  name: 'shows a message when sending a comment fails.',
  args: { commentError: new Error('sample failure') },
}
