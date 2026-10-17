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

function getRenderedOutput(container: HTMLElement, target: NoteVersionComment) {
  const body = screen.getByText(target.body)
  return {
    authorLine: screen.getByText('あなた').parentElement?.textContent ?? null,
    authorLabel: screen.queryByText(target.author_label)?.textContent ?? null,
    initialBadge: container.querySelector('span.size-4')?.textContent ?? null,
    anchorText:
      target.anchor_text == null
        ? null
        : (screen.queryByText(target.anchor_text)?.textContent ?? null),
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
  it.each([
    {
      name: 'shows an unresolved comment without its stored author label or quote',
      resolved: false,
      resolutionStatus: null,
      action: '解決する',
      bodyClassName: 'whitespace-pre-wrap leading-relaxed text-foreground',
    },
    {
      name: 'shows the resolved status separately from its reset action',
      resolved: true,
      resolutionStatus: '✓ 解決済み',
      action: '未解決に戻す',
      bodyClassName:
        'whitespace-pre-wrap leading-relaxed text-muted-foreground line-through decoration-border',
    },
  ])('$name', ({ resolved, resolutionStatus, action, bodyClassName }) => {
    const target = { ...comment, resolved }
    const { container } = render(
      <NoteVersionCommentBody
        comment={target}
        isResolving={false}
        onToggleResolved={() => {}}
      />,
    )

    expect(getRenderedOutput(container, target)).toEqual({
      authorLine: `あなた· ${formatRelative(target.created_at)}`,
      authorLabel: null,
      initialBadge: null,
      anchorText: null,
      resolutionStatus,
      resolutionStatusIsButton: false,
      actions: [{ label: action, disabled: false }],
      body: {
        text: target.body,
        className: bodyClassName,
      },
    })
  })

  it.each([
    {
      name: 'requests resolving an unresolved comment',
      resolved: false,
      action: '解決する',
    },
    {
      name: 'requests reopening a resolved comment',
      resolved: true,
      action: '未解決に戻す',
    },
  ])('$name', async ({ resolved, action }) => {
    const user = userEvent.setup()
    const onToggleResolved = vi.fn()
    const target = { ...comment, resolved }
    render(
      <NoteVersionCommentBody
        comment={target}
        isResolving={false}
        onToggleResolved={onToggleResolved}
      />,
    )

    await user.click(screen.getByRole('button', { name: action }))

    expect(onToggleResolved.mock.calls).toEqual([[target]])
  })
})
