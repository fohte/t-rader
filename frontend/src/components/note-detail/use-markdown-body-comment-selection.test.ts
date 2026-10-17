import {
  act,
  cleanup,
  createEvent,
  fireEvent,
  render,
  screen,
} from '@testing-library/react'
import { createElement, Fragment } from 'react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { MarkdownBodyTextSelectionAction } from '#components/note-detail/markdown-body-text-selection-action'
import type { DiffCommentAnchor } from '#components/note-detail/note-version-line-comment-group'
import { useMarkdownBodyCommentSelection } from '#components/note-detail/use-markdown-body-comment-selection'

const FIRST_LINE = '架空の本文です。対象と対象。'
const SECOND_LINE = '架空の次の行です。'
const THIRD_LINE = '架空の末尾です。'
const SOURCE = [FIRST_LINE, SECOND_LINE, THIRD_LINE].join('\n')

type SelectionHook = ReturnType<typeof useMarkdownBodyCommentSelection>
type HookCapture = ReturnType<typeof vi.fn<(selection: SelectionHook) => void>>

interface SelectionHarnessProps {
  capture: (selection: SelectionHook) => void
  onStartComment: (anchor: DiffCommentAnchor) => void
}

function SelectionHarness({ capture, onStartComment }: SelectionHarnessProps) {
  const selection = useMarkdownBodyCommentSelection({
    enabled: true,
    source: SOURCE,
    onStartComment,
  })
  capture(selection)

  return createElement(
    Fragment,
    null,
    createElement(
      'div',
      {
        ref: selection.rootRef,
        'data-testid': '本文',
        onMouseUp: selection.onMouseUp,
        onPointerMove: selection.onPointerMove,
        onPointerUp: selection.onPointerUp,
        onPointerCancel: selection.onPointerCancel,
      },
      ...[FIRST_LINE, SECOND_LINE, THIRD_LINE].map((text, index) =>
        createElement(
          'div',
          {
            key: text,
            'data-source-block': 'true',
            'data-source-start-line': index + 1,
            'data-source-end-line': index + 1,
            'data-testid': `本文ブロック ${String(index + 1)}`,
          },
          createElement('p', null, text),
        ),
      ),
      createElement(MarkdownBodyTextSelectionAction, {
        selection: selection.textSelectionAction,
        onStartComment: selection.onStartTextComment,
      }),
    ),
    createElement(
      'button',
      { type: 'button', 'data-testid': '本文外' },
      '本文外',
    ),
  )
}

function latestSelection(capture: HookCapture): SelectionHook {
  const selection = capture.mock.lastCall?.[0]
  if (selection == null)
    throw new Error('selection hook が初期化されていません')
  return selection
}

function selectTextRange(start: number, end: number): Selection {
  const paragraph = screen.getByText(FIRST_LINE)
  const textNode = paragraph.firstChild
  if (textNode == null) throw new Error('本文にテキストがありません')

  const range = document.createRange()
  range.setStart(textNode, start)
  range.setEnd(textNode, end)
  Object.defineProperty(range, 'getBoundingClientRect', {
    value: () => new DOMRect(20, 100, 120, 20),
  })
  const selection = window.getSelection()
  if (selection == null) throw new Error('selection を取得できません')
  selection.removeAllRanges()
  selection.addRange(range)
  fireEvent(document, new Event('selectionchange'))
  return selection
}

function selectFirstLine(): Selection {
  return selectTextRange(0, FIRST_LINE.length)
}

afterEach(() => {
  cleanup()
  window.getSelection()?.removeAllRanges()
})

describe('useMarkdownBodyCommentSelection', () => {
  it('clears a gutter drag when the pointer is released outside the body', () => {
    const capture = vi.fn<(selection: SelectionHook) => void>()
    const onStartComment = vi.fn<SelectionHarnessProps['onStartComment']>()
    render(createElement(SelectionHarness, { capture, onStartComment }))

    act(() => {
      latestSelection(capture).onGutterPointerDown({ startLine: 1, endLine: 1 })
    })
    fireEvent.pointerMove(screen.getByTestId('本文ブロック 2'))
    fireEvent.pointerUp(screen.getByTestId('本文外'))
    fireEvent.pointerMove(screen.getByTestId('本文ブロック 3'))

    const getOutsideReleaseResult = () => ({
      selectedRange: latestSelection(capture).selectedRange,
      startedComments: onStartComment.mock.calls,
    })
    expect(getOutsideReleaseResult()).toEqual({
      selectedRange: null,
      startedComments: [],
    })
  })

  it('clears the text selection action after a pointer down outside the body', () => {
    const capture = vi.fn<(selection: SelectionHook) => void>()
    const onStartComment = vi.fn<SelectionHarnessProps['onStartComment']>()
    render(createElement(SelectionHarness, { capture, onStartComment }))
    const selectedText = selectFirstLine()

    fireEvent.pointerDown(screen.getByTestId('本文外'))

    const getOutsideSelectionResult = () => ({
      action: latestSelection(capture).textSelectionAction,
      selectedText: selectedText.toString(),
    })
    expect(getOutsideSelectionResult()).toEqual({
      action: null,
      selectedText: FIRST_LINE,
    })
  })

  it('preserves the text selection when the comment action button is pressed', () => {
    const capture = vi.fn<(selection: SelectionHook) => void>()
    const onStartComment = vi.fn<SelectionHarnessProps['onStartComment']>()
    render(createElement(SelectionHarness, { capture, onStartComment }))
    const selectedText = selectFirstLine()
    const button = screen.getByRole('button', {
      name: '選択した本文にコメント',
    })
    const mouseDown = createEvent.mouseDown(button)

    fireEvent.pointerDown(button)
    act(() => {
      button.dispatchEvent(mouseDown)
    })

    const getButtonPressResult = () => ({
      action: latestSelection(capture).textSelectionAction,
      selectedText: selectedText.toString(),
      mouseDownPrevented: mouseDown.defaultPrevented,
    })
    expect(getButtonPressResult()).toEqual({
      action: {
        text: FIRST_LINE,
        textOffset: 0,
        range: { startLine: 1, endLine: 1 },
        position: { top: 60, left: 20 },
      },
      selectedText: FIRST_LINE,
      mouseDownPrevented: true,
    })
  })

  it('stores the offset of a later repeated phrase in the block', () => {
    const capture = vi.fn<(selection: SelectionHook) => void>()
    const onStartComment = vi.fn<SelectionHarnessProps['onStartComment']>()
    render(createElement(SelectionHarness, { capture, onStartComment }))
    const start = FIRST_LINE.lastIndexOf('対象')
    const selectedText = selectTextRange(start, start + '対象'.length)

    const getSelectionResult = () => ({
      action: latestSelection(capture).textSelectionAction,
      selectedText: selectedText.toString(),
    })
    expect(getSelectionResult()).toEqual({
      action: {
        text: '対象',
        textOffset: start,
        range: { startLine: 1, endLine: 1 },
        position: { top: 60, left: 20 },
      },
      selectedText: '対象',
    })
  })
})
