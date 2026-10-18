import { cleanup, render, screen } from '@testing-library/react'
import type { ExtraProps } from 'react-markdown'
import { afterEach, describe, expect, it } from 'vitest'

import { MarkdownBodyRangeBlock } from '#components/note-detail/markdown-body-range-block'
import {
  MarkdownBodyRangeCommentContext,
  type MarkdownBodyRangeCommentProps,
} from '#components/note-detail/markdown-body-range-comment-context'
import { getDiffCommentAnchorKey } from '#components/note-detail/note-version-line-comment-group'
import type { NoteVersionComment } from '#lib/api/note-version-types'

type MarkdownNode = NonNullable<ExtraProps['node']>

const nestedComment: NoteVersionComment = {
  id: '00000000-0000-0000-0000-000000000401',
  target_kind: 'note_version',
  target_id: '00000000-0000-0000-0000-000000000402',
  parent_id: null,
  body: '子項目へのコメントです。',
  author_kind: 'human',
  author_label: 'レビュー担当者',
  resolved: false,
  anchor_text: '親項目から子項目',
  anchor_side: 'new',
  start_line: 1,
  end_line: 2,
  created_at: '2026-01-12T03:00:00Z',
}
const gutterComment: NoteVersionComment = {
  ...nestedComment,
  start_line: 3,
  end_line: 4,
  anchor_text: '対象の段落です。',
}

interface GutterState {
  comments?: NoteVersionComment[]
  selectedRange?: { startLine: number; endLine: number } | null
}

const rangeCommentProps: MarkdownBodyRangeCommentProps = {
  comments: [nestedComment],
  activeCommentAnchor: null,
  activeCommentKey: null,
  replyingCommentId: null,
  isCreating: false,
  isReplying: false,
  resolvingCommentId: null,
  hasCreateError: false,
  hasReplyError: false,
  hasResolveError: false,
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

function markdownNode(
  tagName: string,
  startLine: number,
  endLine: number,
): MarkdownNode {
  return {
    type: 'element',
    tagName,
    properties: {},
    children: [],
    position: {
      start: { line: startLine, column: 1, offset: startLine * 10 },
      end: { line: endLine, column: 10, offset: endLine * 10 + 9 },
    },
  }
}

function renderNestedList(props: MarkdownBodyRangeCommentProps) {
  return render(
    <MarkdownBodyRangeCommentContext.Provider
      value={{
        props,
        selectedRange: null,
        refChipTexts: {},
        onGutterPointerDown: () => {},
        onGutterClick: () => {},
      }}
    >
      <MarkdownBodyRangeBlock
        node={markdownNode('li', 1, 2)}
        element={
          <ul>
            <li>
              <MarkdownBodyRangeBlock
                node={markdownNode('li', 2, 2)}
                element={<p>子項目</p>}
              />
            </li>
          </ul>
        }
      />
    </MarkdownBodyRangeCommentContext.Provider>,
  )
}

function renderGutterBlock({
  comments = [],
  selectedRange = null,
}: GutterState = {}) {
  return render(
    <MarkdownBodyRangeCommentContext.Provider
      value={{
        props: { ...rangeCommentProps, comments },
        selectedRange,
        refChipTexts: {},
        onGutterPointerDown: () => {},
        onGutterClick: () => {},
      }}
    >
      <MarkdownBodyRangeBlock
        node={markdownNode('p', 3, 4)}
        element={<p>対象の段落です。</p>}
      />
    </MarkdownBodyRangeCommentContext.Provider>,
  )
}

function getGutterState() {
  const button = screen.getByRole('button', {
    name: '行 3 から 4 にコメント',
  })
  const marker = button.firstElementChild
  return {
    label: button.getAttribute('aria-label'),
    marker: marker?.textContent ?? null,
    markerAccessible: marker?.getAttribute('aria-hidden') ?? null,
    markerClassName: marker?.className ?? null,
    buttonIsHighlighted: button.classList.contains('text-primary'),
  }
}

describe('MarkdownBodyRangeBlock', () => {
  it('keeps the gutter visually empty and retains its accessible label', () => {
    renderGutterBlock()

    expect(getGutterState()).toEqual({
      label: '行 3 から 4 にコメント',
      marker: '+',
      markerAccessible: 'true',
      markerClassName:
        'opacity-0 group-[:hover:not(:has([data-source-block]:hover))]:opacity-100 group-[:hover:not(:has([data-source-block]:hover))]:text-primary',
      buttonIsHighlighted: false,
    })
  })

  it('limits the hover marker to the nested list item under the pointer', () => {
    renderNestedList({ ...rangeCommentProps, comments: [] })

    const getGutterMarkerClasses = () =>
      screen
        .getAllByRole('button')
        .filter(
          (button) =>
            button.getAttribute('aria-label')?.includes('にコメント') ?? false,
        )
        .map((button) => ({
          label: button.getAttribute('aria-label'),
          markerClassName: button.firstElementChild?.className ?? null,
        }))
    expect(getGutterMarkerClasses()).toEqual([
      {
        label: '行 2 にコメント',
        markerClassName: 'opacity-0',
      },
      {
        label: '行 1 から 2 にコメント',
        markerClassName: 'opacity-0',
      },
    ])
  })

  it('shows a marker instead of a line number for a commented block', () => {
    renderGutterBlock({ comments: [gutterComment] })

    expect(getGutterState()).toEqual({
      label: '行 3 から 4 にコメント',
      marker: '●',
      markerAccessible: 'true',
      markerClassName: '',
      buttonIsHighlighted: true,
    })
  })

  it('shows a marker instead of a line number for a selected block', () => {
    renderGutterBlock({ selectedRange: { startLine: 3, endLine: 4 } })

    expect(getGutterState()).toEqual({
      label: '行 3 から 4 にコメント',
      marker: '●',
      markerAccessible: 'true',
      markerClassName: '',
      buttonIsHighlighted: true,
    })
  })

  it('renders a nested list comment only below its innermost item', () => {
    renderNestedList(rangeCommentProps)

    const commentText = screen.getByText('子項目へのコメントです。')
    const getCommentPlacement = () => ({
      count: screen.getAllByText('子項目へのコメントです。').length,
      line:
        commentText
          .closest('[data-source-block="true"]')
          ?.getAttribute('data-source-start-line') ?? null,
    })
    expect(getCommentPlacement()).toEqual({ count: 1, line: '2' })
  })

  it('renders a nested list composer only below its innermost item', () => {
    const activeCommentAnchor = {
      side: 'new' as const,
      lineNumber: 2,
      endLineNumber: 2,
      text: '子項目',
    }
    renderNestedList({
      ...rangeCommentProps,
      comments: [],
      activeCommentAnchor,
      activeCommentKey: getDiffCommentAnchorKey(activeCommentAnchor),
    })

    const composer = screen.getByRole('textbox', { name: '行コメント' })
    const getComposerPlacement = () => ({
      count: screen.getAllByRole('textbox', { name: '行コメント' }).length,
      line:
        composer
          .closest('[data-source-block="true"]')
          ?.getAttribute('data-source-start-line') ?? null,
    })
    expect(getComposerPlacement()).toEqual({ count: 1, line: '2' })
  })

  it('keeps the thread visible when the gutter is hidden', () => {
    render(
      <MarkdownBodyRangeCommentContext.Provider
        value={{
          props: rangeCommentProps,
          selectedRange: null,
          refChipTexts: {},
          onGutterPointerDown: () => {},
          onGutterClick: () => {},
        }}
      >
        <MarkdownBodyRangeBlock
          node={markdownNode('p', 2, 2)}
          showGutter={false}
          element={<p>子項目</p>}
        />
      </MarkdownBodyRangeCommentContext.Provider>,
    )

    const getRenderedState = () => ({
      commentCount: screen.getAllByText('子項目へのコメントです。').length,
      gutterCount: screen.queryAllByRole('button', {
        name: '行 2 にコメント',
      }).length,
    })
    expect(getRenderedState()).toEqual({ commentCount: 1, gutterCount: 0 })
  })
})
