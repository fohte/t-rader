import type { Meta, StoryObj } from '@storybook/react-vite'

import {
  NoteVersionDiffView,
  type NoteVersionDiffViewProps,
} from '#components/note-detail/note-version-diff-view'
import type { NoteVersionComment } from '#lib/api/note-version-types'
import { buildNoteVersionDiff } from '#lib/note-version-diff'

const bodyBefore = `# 架空データの確認

観測期間は 4 週間です。
集計結果を確認する。
補足資料を参照する。
`

const bodyAfter = `# 架空データの確認

観測期間は 6 週間です。
集計結果を再確認する。
`

const comment: NoteVersionComment = {
  id: 'fake-comment-a',
  target_id: 'fake-version-a',
  target_kind: 'note_version',
  parent_id: null,
  body: '集計条件の根拠を追記してください。',
  author_kind: 'human',
  author_label: 'reviewer',
  resolved: false,
  anchor_text: '集計結果を再確認する。',
  anchor_side: 'new',
  start_line: 4,
  end_line: 4,
  created_at: '2026-01-12T03:00:00Z',
}

const replies: NoteVersionComment[] = [
  comment,
  {
    ...comment,
    id: 'fake-comment-b',
    parent_id: comment.id,
    body: '確認して追記します。',
    author_kind: 'llm',
    author_label: 'analyst',
    resolved: true,
    created_at: '2026-01-12T03:05:00Z',
  },
]

const args: NoteVersionDiffViewProps = {
  rows: buildNoteVersionDiff(bodyBefore, bodyAfter),
  comments: [],
  mode: 'one-column',
  activeCommentKey: null,
  replyingCommentId: null,
  isCreating: false,
  isReplying: false,
  resolvingCommentId: null,
  hasCreateError: false,
  hasReplyError: false,
  isCommentsPending: false,
  hasCommentsError: false,
  onModeChange: () => {},
  onStartComment: () => {},
  onCancelComment: () => {},
  onCreateComment: () => {},
  onStartReply: () => {},
  onCancelReply: () => {},
  onReply: () => {},
  onToggleResolved: () => {},
}

const meta = {
  title: 'NoteDetail/NoteVersionDiffView',
  component: NoteVersionDiffView,
  parameters: { layout: 'padded' },
  args,
  decorators: [
    (Story) => (
      <div className="max-w-5xl bg-background text-foreground">
        <Story />
      </div>
    ),
  ],
} satisfies Meta<typeof NoteVersionDiffView>

export default meta
type Story = StoryObj<typeof meta>

export const OneColumn: Story = {}

export const TwoColumns: Story = { args: { mode: 'two-column' } }

export const ThreadAndReply: Story = {
  args: {
    comments: replies,
  },
}

export const ReplyFormOpen: Story = {
  args: {
    comments: replies,
    replyingCommentId: comment.id,
  },
}

export const NewLineCommentFormOpen: Story = {
  args: {
    activeCommentKey: 'new:4',
  },
}

export const EmptyBodyWithLineAnchor: Story = {
  args: {
    rows: buildNoteVersionDiff(null, ''),
  },
}

export const Empty: Story = { args: { rows: [] } }
