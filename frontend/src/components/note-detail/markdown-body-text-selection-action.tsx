import { Button } from '@fohte/ui/button'
import type { CSSProperties, MouseEvent } from 'react'

import type { TextSelectionAction } from '#components/note-detail/use-markdown-body-comment-selection'

type SelectionActionStyle = CSSProperties & {
  '--markdown-body-selection-action-left': string
  '--markdown-body-selection-action-top': string
}

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
  const positionStyle: SelectionActionStyle = {
    '--markdown-body-selection-action-left': `${String(selection.position.left)}px`,
    '--markdown-body-selection-action-top': `${String(selection.position.top)}px`,
  }

  return (
    <div
      className="fixed top-(--markdown-body-selection-action-top) left-(--markdown-body-selection-action-left) z-50"
      style={positionStyle}
    >
      <Button
        type="button"
        variant="plain"
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
      >
        <span className="px-2 py-1 font-mono text-2xs">💬 コメント</span>
      </Button>
    </div>
  )
}
