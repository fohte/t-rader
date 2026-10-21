import { Button } from '@fohte/ui/button'
import { Chip } from '@fohte/ui/chip'

import type { NoteVersionComment } from '#lib/api/note-version-types'
import { formatRelative } from '#lib/note-utils'
import { cn } from '#lib/utils'

export interface NoteVersionCommentBodyProps {
  comment: NoteVersionComment
  isResolving: boolean
  onToggleResolved: (comment: NoteVersionComment) => void
}

export function NoteVersionCommentBody({
  comment,
  isResolving,
  onToggleResolved,
}: NoteVersionCommentBodyProps) {
  const isLLM = comment.author_kind === 'llm'
  const resolutionControls = (
    <>
      {comment.resolved && (
        <span className="text-status-approved">✓ 解決済み</span>
      )}
      <Button
        type="button"
        variant="plain"
        onClick={() => {
          onToggleResolved(comment)
        }}
        disabled={isResolving}
      >
        {comment.resolved ? '未解決に戻す' : '解決する'}
      </Button>
    </>
  )

  return (
    <div className="space-y-1.5">
      <div className="flex flex-wrap items-center gap-2 font-mono text-2xs">
        {isLLM ? (
          <>
            <span className="font-bold text-foreground">
              {comment.author_label}
            </span>
            <Chip>LLM</Chip>
            <div className="ml-auto flex items-center gap-2">
              {resolutionControls}
              <span className="text-muted-foreground">
                {formatRelative(comment.created_at)}
              </span>
            </div>
          </>
        ) : (
          <>
            <div className="flex items-center gap-1">
              <span className="font-bold text-foreground">あなた</span>
              <span className="text-muted-foreground">
                · {formatRelative(comment.created_at)}
              </span>
            </div>
            <div className="ml-auto flex items-center gap-2">
              {resolutionControls}
            </div>
          </>
        )}
      </div>
      <p
        className={cn(
          'whitespace-pre-wrap leading-relaxed',
          comment.resolved
            ? 'text-muted-foreground line-through decoration-border'
            : 'text-foreground',
        )}
      >
        {comment.body}
      </p>
    </div>
  )
}
