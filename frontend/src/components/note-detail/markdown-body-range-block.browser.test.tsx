import '#index.css'

import { cleanup, render } from '@testing-library/react'
import { afterEach, describe, expect, it } from 'vitest'
import { page } from 'vitest/browser'

import { MarkdownBody } from '#components/note-detail/markdown-body'
import type { MarkdownBodyRangeCommentProps } from '#components/note-detail/markdown-body-range-comment-context'

const emptyRangeComments: MarkdownBodyRangeCommentProps = {
  comments: [],
  activeCommentAnchor: null,
  activeCommentKey: null,
  replyingCommentId: null,
  isCreating: false,
  isReplying: false,
  resolvingCommentId: null,
  hasCreateError: false,
  hasReplyError: false,
  isCommentsPending: false,
  hasCommentsError: false,
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

describe('MarkdownBodyRangeBlock hover gutter', () => {
  it.each([
    {
      name: '親項目に hover すると親だけに表示する',
      hoverTarget: '親項目',
      expected: [
        { line: '1', opacity: '1' },
        { line: '2', opacity: '0' },
      ],
    },
    {
      name: '子項目に hover すると子だけに表示する',
      hoverTarget: '子項目',
      expected: [
        { line: '1', opacity: '0' },
        { line: '2', opacity: '1' },
      ],
    },
  ])('$name', async ({ hoverTarget, expected }) => {
    render(
      <MarkdownBody
        source={'- 親項目\n    - 子項目'}
        rangeComments={emptyRangeComments}
      />,
    )

    await page.getByText(hoverTarget).hover()

    const markerStates = [
      ...document.querySelectorAll<HTMLLIElement>(
        'li[data-source-block="true"]',
      ),
    ].map((item) => {
      const marker = item.querySelector(
        ':scope > button[data-source-gutter-button="true"] > span[data-source-gutter-marker="true"]',
      )
      return {
        line: item.getAttribute('data-source-start-line'),
        opacity: marker == null ? null : getComputedStyle(marker).opacity,
      }
    })

    expect(markerStates).toEqual(expected)
  })
})
