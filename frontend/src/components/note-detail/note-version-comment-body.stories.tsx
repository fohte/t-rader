import type { Meta, StoryObj } from '@storybook/react-vite'

import { NoteVersionCommentBody } from '#components/note-detail/note-version-comment-body'
import type { NoteVersionComment } from '#lib/api/note-version-types'

const comment: NoteVersionComment = {
  id: 'sample-comment',
  target_kind: 'note_version',
  target_id: 'sample-version',
  parent_id: null,
  body: '数値の根拠を補足してください。',
  author_kind: 'human',
  author_label: 'human-author',
  resolved: false,
  anchor_text: '保存された引用文',
  anchor_side: 'new',
  start_line: 4,
  end_line: 4,
  created_at: '2026-01-12T03:00:00Z',
}

const meta = {
  title: 'NoteDetail/NoteVersionCommentBody',
  component: NoteVersionCommentBody,
  parameters: { layout: 'padded' },
  args: {
    comment,
    isResolving: false,
    onToggleResolved: () => {},
  },
  decorators: [
    (Story) => (
      <div className="max-w-3xl border border-border bg-card p-4 text-foreground">
        <Story />
      </div>
    ),
  ],
} satisfies Meta<typeof NoteVersionCommentBody>

export default meta
type Story = StoryObj<typeof meta>

export const UnresolvedHumanComment: Story = {
  name: 'shows an unresolved comment written by you.',
}

export const ResolvedHumanComment: Story = {
  name: 'shows a resolved comment with a separate reset action.',
  args: {
    comment: {
      ...comment,
      resolved: true,
    },
  },
}

export const ResolvedAgentReply: Story = {
  name: 'shows a resolved assistant reply with its LLM label.',
  args: {
    comment: {
      ...comment,
      id: 'sample-reply',
      parent_id: comment.id,
      body: '確認して補足しました。',
      author_kind: 'llm',
      author_label: 'scenario-agent',
      resolved: true,
    },
  },
}
