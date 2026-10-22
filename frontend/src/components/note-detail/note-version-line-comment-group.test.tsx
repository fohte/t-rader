import { cleanup, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it, vi } from 'vitest'

import {
  type DiffCommentAnchor,
  LineCommentGroup,
} from '#components/note-detail/note-version-line-comment-group'
import type { NoteVersionComment } from '#lib/api/note-version-types'

const anchor: DiffCommentAnchor = {
  side: 'new',
  lineNumber: 3,
  text: '架空の変更行',
}

const comment: NoteVersionComment = {
  id: 'fake-comment-a',
  target_id: 'fake-version-a',
  target_kind: 'note_version',
  parent_id: null,
  body: '補足してください。',
  author_kind: 'human',
  author_label: 'reviewer',
  resolved: false,
  anchor_text: anchor.text,
  anchor_side: anchor.side,
  start_line: anchor.lineNumber,
  end_line: anchor.lineNumber,
  created_at: '2026-01-12T03:00:00Z',
}

afterEach(cleanup)

describe('LineCommentGroup', () => {
  it('submits a trimmed comment for the selected line', async () => {
    const user = userEvent.setup()
    const onCreateComment =
      vi.fn<(anchor: DiffCommentAnchor, body: string) => void>()
    render(
      <LineCommentGroup
        comments={[]}
        anchor={anchor}
        anchorKey="new:3"
        activeCommentKey="new:3"
        replyingCommentId={null}
        isCreating={false}
        isReplying={false}
        resolvingCommentId={null}
        hasCreateError={false}
        hasReplyError={false}
        onStartComment={() => {}}
        onCancelComment={() => {}}
        onCreateComment={onCreateComment}
        onStartReply={() => {}}
        onCancelReply={() => {}}
        onReply={() => {}}
        onToggleResolved={() => {}}
      />,
    )

    await user.type(
      screen.getByRole('textbox', { name: '行コメント' }),
      '  補足します  ',
    )
    await user.click(screen.getByRole('button', { name: '送信' }))

    expect(onCreateComment.mock.calls).toEqual([[anchor, '補足します']])
  })

  it('submits a trimmed reply to the selected comment', async () => {
    const user = userEvent.setup()
    const onReply = vi.fn<(commentId: string, body: string) => void>()
    render(
      <LineCommentGroup
        comments={[comment]}
        anchor={anchor}
        anchorKey="new:3"
        activeCommentKey={null}
        replyingCommentId={comment.id}
        isCreating={false}
        isReplying={false}
        resolvingCommentId={null}
        hasCreateError={false}
        hasReplyError={false}
        onStartComment={() => {}}
        onCancelComment={() => {}}
        onCreateComment={() => {}}
        onStartReply={() => {}}
        onCancelReply={() => {}}
        onReply={onReply}
        onToggleResolved={() => {}}
      />,
    )

    await user.type(
      screen.getByRole('textbox', { name: '返信' }),
      '  確認しました  ',
    )
    await user.click(screen.getByRole('button', { name: '送信' }))

    expect(onReply.mock.calls).toEqual([[comment.id, '確認しました']])
  })
})
