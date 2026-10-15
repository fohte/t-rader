import { cleanup, render } from '@testing-library/react'
import { afterEach, describe, expect, it } from 'vitest'

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

describe('NoteVersionDiffView', () => {
  it('marks all lines in a range and renders its thread only below the last line', () => {
    const { container } = render(<NoteVersionDiffView {...props} />)
    expect(getRangeCommentOutput(container)).toEqual({
      rangeLines: ['3', '4', '5'],
      threadLines: ['5'],
    })
  })
})
