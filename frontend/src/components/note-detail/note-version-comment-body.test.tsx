import { cleanup, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { NoteVersionCommentBody } from '#components/note-detail/note-version-comment-body'
import type { NoteVersionComment } from '#lib/api/note-version-types'
import { formatRelative } from '#lib/note-utils'

const comment: NoteVersionComment = {
  id: 'test-comment',
  target_kind: 'note_version',
  target_id: 'test-version',
  parent_id: null,
  body: '数値の根拠を補足してください。',
  author_kind: 'human',
  author_label: 'human-author',
  resolved: false,
  anchor_text: '保存された引用文',
  anchor_side: 'new',
  start_line: 4,
  end_line: 4,
  created_at: '2025-02-03T04:05:06Z',
}

afterEach(cleanup)

function getRenderedOutput(container: HTMLElement) {
  const body = screen.getByText(comment.body)
  return {
    authorLine: screen.getByText('あなた').parentElement?.textContent ?? null,
    authorLabel: screen.queryByText(comment.author_label)?.textContent ?? null,
    initialBadge: container.querySelector('span.size-4')?.textContent ?? null,
    anchorText:
      comment.anchor_text == null
        ? null
        : (screen.queryByText(comment.anchor_text)?.textContent ?? null),
    resolutionStatus: screen.queryByText('✓ 解決済み')?.textContent ?? null,
    resolutionStatusIsButton:
      screen.queryByRole('button', { name: '✓ 解決済み' }) !== null,
    actions: Array.from(container.querySelectorAll('button')).map((button) => ({
      label: button.textContent,
      disabled: button.disabled,
    })),
    body: {
      text: body.textContent,
      className: body.className,
    },
  }
}

describe('NoteVersionCommentBody', () => {
  it('shows an unresolved human comment without its stored author label or quote', () => {
    const { container } = render(
      <NoteVersionCommentBody
        comment={comment}
        isResolving={false}
        onToggleResolved={() => {}}
      />,
    )

    expect(getRenderedOutput(container)).toEqual({
      authorLine: `あなた· ${formatRelative(comment.created_at)}`,
      authorLabel: null,
      initialBadge: null,
      anchorText: null,
      resolutionStatus: null,
      resolutionStatusIsButton: false,
      actions: [{ label: '解決する', disabled: false }],
      body: {
        text: comment.body,
        className: 'whitespace-pre-wrap leading-relaxed text-foreground',
      },
    })
  })

  it('requests resolving an unresolved comment', async () => {
    const user = userEvent.setup()
    const onToggleResolved = vi.fn()
    render(
      <NoteVersionCommentBody
        comment={comment}
        isResolving={false}
        onToggleResolved={onToggleResolved}
      />,
    )

    await user.click(screen.getByRole('button', { name: '解決する' }))

    expect(onToggleResolved.mock.calls).toEqual([[comment]])
  })

  it('shows the resolved status separately from its reset action', () => {
    const resolvedComment = { ...comment, resolved: true }
    const { container } = render(
      <NoteVersionCommentBody
        comment={resolvedComment}
        isResolving={false}
        onToggleResolved={() => {}}
      />,
    )

    expect(getRenderedOutput(container)).toEqual({
      authorLine: `あなた· ${formatRelative(comment.created_at)}`,
      authorLabel: null,
      initialBadge: null,
      anchorText: null,
      resolutionStatus: '✓ 解決済み',
      resolutionStatusIsButton: false,
      actions: [{ label: '未解決に戻す', disabled: false }],
      body: {
        text: comment.body,
        className:
          'whitespace-pre-wrap leading-relaxed text-muted-foreground line-through decoration-border',
      },
    })
  })

  it('requests reopening a resolved comment', async () => {
    const user = userEvent.setup()
    const onToggleResolved = vi.fn()
    const resolvedComment = { ...comment, resolved: true }
    render(
      <NoteVersionCommentBody
        comment={resolvedComment}
        isResolving={false}
        onToggleResolved={onToggleResolved}
      />,
    )

    await user.click(screen.getByRole('button', { name: '未解決に戻す' }))

    expect(onToggleResolved.mock.calls).toEqual([[resolvedComment]])
  })
})
