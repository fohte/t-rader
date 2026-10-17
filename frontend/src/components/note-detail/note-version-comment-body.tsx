import type { NoteVersionComment } from '#lib/api/note-version-types'
import { formatRelative } from '#lib/note-utils'

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
      <button
        type="button"
        onClick={() => {
          onToggleResolved(comment)
        }}
        disabled={isResolving}
        className={
          comment.resolved
            ? 'text-muted-foreground underline-offset-2 hover:text-primary hover:underline disabled:opacity-50'
            : 'border border-border px-1.5 py-0.5 text-2xs text-muted-foreground hover:border-primary hover:text-primary disabled:opacity-50'
        }
      >
        {comment.resolved ? '未解決に戻す' : '解決する'}
      </button>
    </>
  )

  return (
    <div className="space-y-1.5">
      <div className="flex flex-wrap items-center gap-2 font-mono text-2xs">
        {isLLM ? (
          <>
            <span className="inline-flex size-4 items-center justify-center border border-primary text-2xs font-bold text-primary">
              &gt;
            </span>
            <span className="font-bold text-foreground">
              {comment.author_label}
            </span>
            <span className="text-muted-foreground">LLM</span>
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
        className={`whitespace-pre-wrap leading-relaxed ${comment.resolved ? 'text-muted-foreground line-through decoration-border' : 'text-foreground'}`}
      >
        {comment.body}
      </p>
    </div>
  )
}
