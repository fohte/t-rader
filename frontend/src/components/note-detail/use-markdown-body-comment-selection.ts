import {
  type PointerEvent,
  type RefObject,
  useCallback,
  useEffect,
  useRef,
  useState,
} from 'react'

import type { DiffCommentAnchor } from '#components/note-detail/note-version-line-comment-group'

export interface SourceLineRange {
  startLine: number
  endLine: number
}

export interface TextSelectionAction {
  text: string
  range: SourceLineRange
  position: { top: number; left: number }
}

interface UseMarkdownBodyCommentSelectionOptions {
  enabled: boolean
  source: string
  onStartComment: (anchor: DiffCommentAnchor) => void
}

interface DragSelection {
  start: SourceLineRange
  hasMoved: boolean
}

interface UseMarkdownBodyCommentSelectionResult {
  rootRef: RefObject<HTMLDivElement | null>
  selectedRange: SourceLineRange | null
  textSelectionAction: TextSelectionAction | null
  onMouseUp: () => void
  onTouchEnd: () => void
  onPointerMove: (event: PointerEvent<HTMLDivElement>) => void
  onPointerUp: () => void
  onPointerCancel: () => void
  onGutterPointerDown: (range: SourceLineRange) => void
  onGutterClick: (range: SourceLineRange, shiftKey: boolean) => void
  onStartTextComment: () => void
  clearBlockSelection: () => void
}

function rangeForElement(element: Element | null): SourceLineRange | null {
  const block = element?.closest<HTMLElement>('[data-source-block="true"]')
  const startLine = Number(block?.dataset.sourceStartLine)
  const endLine = Number(block?.dataset.sourceEndLine)
  if (!Number.isInteger(startLine) || !Number.isInteger(endLine)) return null
  if (startLine < 1 || endLine < startLine) return null
  return { startLine, endLine }
}

function elementForNode(node: Node | null): Element | null {
  if (node instanceof Element) return node
  return node?.parentElement ?? null
}

function mergeRanges(a: SourceLineRange, b: SourceLineRange): SourceLineRange {
  return {
    startLine: Math.min(a.startLine, b.startLine),
    endLine: Math.max(a.endLine, b.endLine),
  }
}

function rangesEqual(
  a: SourceLineRange | null,
  b: SourceLineRange | null,
): boolean {
  return a?.startLine === b?.startLine && a?.endLine === b?.endLine
}

export function useMarkdownBodyCommentSelection({
  enabled,
  source,
  onStartComment,
}: UseMarkdownBodyCommentSelectionOptions): UseMarkdownBodyCommentSelectionResult {
  const rootRef = useRef<HTMLDivElement>(null)
  const [selectedRange, setSelectedRange] = useState<SourceLineRange | null>(
    null,
  )
  const [textSelectionAction, setTextSelectionAction] =
    useState<TextSelectionAction | null>(null)
  const selectedRangeRef = useRef<SourceLineRange | null>(null)
  const dragSelectionRef = useRef<DragSelection | null>(null)
  const lastClickedRangeRef = useRef<SourceLineRange | null>(null)
  const suppressNextClickRef = useRef(false)

  const updateSelectedRange = useCallback((range: SourceLineRange | null) => {
    selectedRangeRef.current = range
    setSelectedRange(range)
  }, [])

  const blockText = useCallback(
    (range: SourceLineRange): string =>
      source
        .split('\n')
        .slice(range.startLine - 1, range.endLine)
        .join('\n'),
    [source],
  )

  const startComment = useCallback(
    (range: SourceLineRange, text = blockText(range)): void => {
      onStartComment({
        side: 'new',
        lineNumber: range.startLine,
        endLineNumber: range.endLine,
        text,
      })
      updateSelectedRange(range)
      setTextSelectionAction(null)
      window.getSelection()?.removeAllRanges()
    },
    [blockText, onStartComment, updateSelectedRange],
  )

  const updateTextSelection = useCallback(
    (preserveExisting = false) => {
      if (!enabled || rootRef.current == null) return
      const selection = document.getSelection()
      if (
        selection == null ||
        selection.isCollapsed ||
        selection.toString() === '' ||
        !rootRef.current.contains(selection.anchorNode) ||
        !rootRef.current.contains(selection.focusNode)
      ) {
        if (!preserveExisting) setTextSelectionAction(null)
        return
      }

      const selected = selection.getRangeAt(0)
      const startRange = rangeForElement(
        elementForNode(selected.startContainer),
      )
      const endRange = rangeForElement(elementForNode(selected.endContainer))
      if (startRange == null || endRange == null) {
        setTextSelectionAction(null)
        return
      }
      const rect = selected.getBoundingClientRect()
      setTextSelectionAction({
        text: selection.toString(),
        range: mergeRanges(startRange, endRange),
        position: {
          top: rect.top < 44 ? rect.bottom + 8 : rect.top - 40,
          left: Math.max(8, rect.left),
        },
      })
    },
    [enabled],
  )

  useEffect(() => {
    if (!enabled) return
    const onSelectionChange = (): void => {
      updateTextSelection(true)
    }
    document.addEventListener('selectionchange', onSelectionChange)
    return () => {
      document.removeEventListener('selectionchange', onSelectionChange)
    }
  }, [enabled, updateTextSelection])

  const onMouseUp = useCallback((): void => {
    updateTextSelection()
  }, [updateTextSelection])

  const onTouchEnd = useCallback((): void => {
    updateTextSelection()
  }, [updateTextSelection])

  const onPointerMove = useCallback(
    (event: PointerEvent<HTMLDivElement>): void => {
      const drag = dragSelectionRef.current
      if (drag == null) return
      const pointedElement =
        typeof document.elementFromPoint === 'function'
          ? document.elementFromPoint(event.clientX, event.clientY)
          : null
      const hoveredRange = rangeForElement(
        pointedElement ??
          (event.target instanceof Node ? elementForNode(event.target) : null),
      )
      if (hoveredRange == null) return
      const nextRange = mergeRanges(drag.start, hoveredRange)
      if (rangesEqual(selectedRangeRef.current, nextRange)) return
      drag.hasMoved = true
      updateSelectedRange(nextRange)
    },
    [updateSelectedRange],
  )

  const onPointerUp = useCallback((): void => {
    const drag = dragSelectionRef.current
    if (drag == null) return
    dragSelectionRef.current = null
    if (!drag.hasMoved) return
    const range = selectedRangeRef.current ?? drag.start
    lastClickedRangeRef.current = drag.start
    suppressNextClickRef.current = true
    startComment(range)
  }, [startComment])

  const onPointerCancel = useCallback((): void => {
    dragSelectionRef.current = null
    updateSelectedRange(null)
  }, [updateSelectedRange])

  const onGutterPointerDown = useCallback(
    (range: SourceLineRange): void => {
      dragSelectionRef.current = { start: range, hasMoved: false }
      updateSelectedRange(range)
    },
    [updateSelectedRange],
  )

  const onGutterClick = useCallback(
    (range: SourceLineRange, shiftKey: boolean): void => {
      if (suppressNextClickRef.current) {
        suppressNextClickRef.current = false
        return
      }
      const previousRange = lastClickedRangeRef.current
      const selectedRange =
        shiftKey && previousRange != null
          ? mergeRanges(previousRange, range)
          : range
      lastClickedRangeRef.current = range
      startComment(selectedRange)
    },
    [startComment],
  )

  const startTextComment = useCallback((): void => {
    if (textSelectionAction == null) return
    startComment(textSelectionAction.range, textSelectionAction.text)
  }, [startComment, textSelectionAction])

  const clearBlockSelection = useCallback((): void => {
    updateSelectedRange(null)
    lastClickedRangeRef.current = null
  }, [updateSelectedRange])

  return {
    rootRef,
    selectedRange,
    textSelectionAction,
    onMouseUp,
    onTouchEnd,
    onPointerMove,
    onPointerUp,
    onPointerCancel,
    onGutterPointerDown,
    onGutterClick,
    onStartTextComment: startTextComment,
    clearBlockSelection,
  }
}
