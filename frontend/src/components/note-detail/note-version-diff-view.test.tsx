import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import {
  NoteVersionDiffView,
  type NoteVersionDiffViewProps,
} from '#components/note-detail/note-version-diff-view'
import type { NoteVersionComment } from '#lib/api/note-version-types'
import { buildNoteVersionDiff } from '#lib/note-version-diff'

const body = 'one\ntwo\nthree\nfour\nfive\nsix\nseven'
const rangeComment: NoteVersionComment = {
  id: 'fake-comment-a',
  target_id: 'fake-version-a',
  target_kind: 'note_version',
  parent_id: null,
  body: '補足をお願いします。',
  author_kind: 'human',
  author_label: 'reviewer',
  resolved: false,
  anchor_text: 'three\nfour\nfive',
  anchor_side: 'new',
  start_line: 3,
  end_line: 5,
  created_at: '2026-01-12T03:00:00Z',
}

const props: NoteVersionDiffViewProps = {
  rows: buildNoteVersionDiff(body, body),
  comments: [rangeComment],
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

afterEach(() => {
  cleanup()
})

function getRangeCommentOutput(container: HTMLElement) {
  const rangeLines = [...container.querySelectorAll('.border-l-2')]
    .filter((cell) => cell.className.includes('ring-1'))
    .map((cell) => cell.querySelector('.w-8')?.textContent ?? null)
  const threadLines = [...container.querySelectorAll('p')]
    .filter((paragraph) => paragraph.textContent === rangeComment.body)
    .map(
      (paragraph) =>
        paragraph.closest('.border-l-2')?.querySelector('.w-8')?.textContent ??
        null,
    )

  return { rangeLines, threadLines }
}

function getModeChangeOutput(
  modeChanges: readonly unknown[][],
  selectedModes: Array<string | null>,
) {
  return { modeChanges, selectedModes }
}

describe('NoteVersionDiffView', () => {
  it('marks all lines in a range and renders its thread only below the last line', () => {
    const { container } = render(<NoteVersionDiffView {...props} />)
    expect(getRangeCommentOutput(container)).toEqual({
      rangeLines: ['3', '4', '5'],
      threadLines: ['5'],
    })
  })

  it('changes the diff layout through the shared segmented control', () => {
    const onModeChange = vi.fn()
    const { rerender } = render(
      <NoteVersionDiffView {...props} onModeChange={onModeChange} />,
    )

    fireEvent.click(screen.getByRole('button', { name: '分割' }))
    rerender(
      <NoteVersionDiffView
        {...props}
        mode="two-column"
        onModeChange={onModeChange}
      />,
    )

    expect(
      getModeChangeOutput(
        onModeChange.mock.calls,
        ['統合', '分割'].map((name) =>
          screen.getByRole('button', { name }).getAttribute('aria-pressed'),
        ),
      ),
    ).toEqual({
      modeChanges: [['two-column']],
      selectedModes: ['false', 'true'],
    })
  }, 15_000)
})
