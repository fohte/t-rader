import type { MouseEvent } from 'react'

import type { TextSelectionAction } from '#components/note-detail/use-markdown-body-comment-selection'

export interface MarkdownBodyTextSelectionActionProps {
  selection: TextSelectionAction | null
  onStartComment: () => void
}

export function MarkdownBodyTextSelectionAction({
  selection,
  onStartComment,
}: MarkdownBodyTextSelectionActionProps) {
  if (selection == null) return null

  const preserveSelection = (event: MouseEvent<HTMLButtonElement>): void => {
    event.preventDefault()
  }

  return (
    <button
      type="button"
      aria-label="選択した本文にコメント"
      onMouseDown={preserveSelection}
      onMouseUp={(event) => {
        event.stopPropagation()
      }}
      onTouchEnd={(event) => {
        event.stopPropagation()
      }}
      onClick={(event) => {
        event.preventDefault()
        event.stopPropagation()
        onStartComment()
      }}
      className="fixed z-50 border border-primary bg-card px-2 py-1 font-mono text-2xs text-primary shadow-md hover:bg-primary/10"
      style={{ left: selection.position.left, top: selection.position.top }}
    >
      💬 コメント
    </button>
  )
}
