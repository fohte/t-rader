import type { Meta, StoryObj } from '@storybook/react-vite'
import { RouterProvider } from '@tanstack/react-router'
import { fn } from 'storybook/test'

import { AnnotationDetail } from '#components/annotations/annotation-detail'
import {
  type ReviewQueueItem,
  reviewQueueModel,
} from '#components/review/review-queue-model'
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

const unreadReviewItems: ReviewQueueItem[] = [
  {
    kind: 'annotation',
    id: unreadAnnotation.id,
    title: unreadAnnotation.text,
    createdAt: unreadAnnotation.created_at,
    targetSymbol: unreadAnnotation.target_symbol,
    timestamp: unreadAnnotation.timestamp,
    price: unreadAnnotation.price ?? null,
  },
  {
    kind: 'note',
    id: '00000000-0000-0000-0000-000000000018',
    noteId: '00000000-0000-0000-0000-000000000019',
    versionNo: 2,
    title: '確認用のメモ',
    createdAt: '2026-10-13T01:22:00Z',
    noteKind: null,
    createdByKind: 'llm',
    unreadVersionCount: 1,
  },
]

const reviewNavigationSnapshot =
  reviewQueueModel.createNavigationSnapshot(unreadReviewItems)
const unreadReviewNavigation = {
  items: unreadReviewItems,
  snapshot: reviewNavigationSnapshot,
  currentItemKey: `annotation:${unreadAnnotation.id}`,
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
            paths: [
              '/annotations',
              '/annotations/$annoId',
              '/notes',
              '/notes/$noteId',
              '/review',
              '/stocks/$stockId',
            ],
          },
        )}
      />
    ),
  ],
  args: {
    annotation: unreadAnnotation,
    isPending: false,
    reviewNavigation: unreadReviewNavigation,
    comments: [],
    history: [],
    stockName: 'サンプル銘柄',
    commentDraft: '',
    rejectionReason: '',
    isApprovalPending: false,
    isRejectionPending: false,
    isCommentPending: false,
    approvalError: null,
    rejectionError: null,
    commentError: null,
    onApprove: fn(),
    onReject: fn(),
    onRejectionReasonChange: fn(),
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

export const ApprovalPending: Story = {
  name: 'disables approval while it is being processed.',
  args: { isApprovalPending: true },
}

export const RejectionPending: Story = {
  name: 'disables rejection while it is being processed.',
  args: { isRejectionPending: true },
}

export const ApprovedWithRelatedItems: Story = {
  name: 'shows an approved annotation with comments and a linked note.',
  args: {
    annotation: approvedAnnotation,
    reviewNavigation: {
      ...unreadReviewNavigation,
      currentItemKey: `annotation:${approvedAnnotation.id}`,
    },
    comments: [approvedComment],
    history: [historyEntry],
  },
}

export const RejectedWithComments: Story = {
  name: 'shows a rejected annotation with comments and no linked note.',
  args: {
    annotation: rejectedAnnotation,
    reviewNavigation: {
      ...unreadReviewNavigation,
      currentItemKey: `annotation:${rejectedAnnotation.id}`,
    },
    comments: [rejectedComment],
  },
}

export const RejectionReasonEntered: Story = {
  name: 'shows an optional rejection reason before the decision.',
  args: { rejectionReason: '確認が必要な点を追記してください。' },
}

export const NoUnreadItems: Story = {
  name: 'shows the review link when no unread items remain.',
  args: {
    annotation: approvedAnnotation,
    reviewNavigation: {
      ...unreadReviewNavigation,
      items: [],
      currentItemKey: `annotation:${approvedAnnotation.id}`,
    },
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

export const CommentDraftEntered: Story = {
  name: 'enables comment submission after text is entered.',
  args: { commentDraft: 'サンプルコメント' },
}
