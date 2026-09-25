import type { Meta, StoryObj } from '@storybook/react-vite'

import { LineCommentGroup } from '#components/note-detail/note-version-line-comment-group'
import type { NoteVersionComment } from '#lib/api/note-version-types'

const comment: NoteVersionComment = {
  id: 'sample-comment',
  target_kind: 'note_version',
  target_id: 'sample-version',
  parent_id: null,
  body: '集計条件の根拠を補足してください。',
  author_kind: 'human',
  author_label: 'レビュー担当者',
  resolved: false,
  anchor_text: '集計結果を再確認する。',
  anchor_side: 'new',
  start_line: 4,
  end_line: 4,
  created_at: '2026-01-12T03:00:00Z',
}

const thread: NoteVersionComment[] = [
  comment,
  {
    ...comment,
    id: 'sample-reply',
    parent_id: comment.id,
    body: '確認して補足しました。',
    author_kind: 'llm',
    author_label: 'analyst',
    resolved: true,
    created_at: '2026-01-12T03:05:00Z',
  },
]

const meta = {
  title: 'NoteDetail/NoteVersionLineCommentGroup',
  component: LineCommentGroup,
  parameters: { layout: 'padded' },
  args: {
    comments: thread,
    anchor: { side: 'new', lineNumber: 4, text: '集計結果を再確認する。' },
    anchorKey: 'new:4',
    allowNewComment: true,
    activeCommentKey: null,
    replyingCommentId: null,
    isCreating: false,
    isReplying: false,
    resolvingCommentId: null,
    hasCreateError: false,
    hasReplyError: false,
    onStartComment: () => {},
    onCancelComment: () => {},
    onCreateComment: () => {},
    onStartReply: () => {},
    onCancelReply: () => {},
    onReply: () => {},
    onToggleResolved: () => {},
  },
  decorators: [
    (Story) => (
      <div className="max-w-3xl border border-border bg-card p-4 text-foreground">
        <Story />
      </div>
    ),
  ],
} satisfies Meta<typeof LineCommentGroup>

export default meta
type Story = StoryObj<typeof meta>

export const Thread: Story = {
  name: 'shows a comment thread anchored to a changed line.',
}

export const NewCommentFormOpen: Story = {
  name: 'shows a new comment form for a changed line.',
  args: { activeCommentKey: 'new:4' },
}

export const ReplyFormOpen: Story = {
  name: 'shows a reply form for an existing comment.',
  args: { replyingCommentId: comment.id },
}

export const ThreadWithoutAnchor: Story = {
  name: 'shows a comment thread without a line anchor.',
  args: {
    comments: thread.map((item) => ({
      ...item,
      anchor_text: null,
      anchor_side: null,
      start_line: null,
      end_line: null,
    })),
    anchor: null,
    anchorKey: null,
    allowNewComment: false,
  },
}
