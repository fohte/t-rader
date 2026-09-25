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
  return (
    <div className="space-y-1.5">
      <div className="flex flex-wrap items-center gap-2 font-mono text-2xs">
        <span
          className={`inline-flex size-4 items-center justify-center border text-2xs font-bold ${isLLM ? 'border-primary text-primary' : 'border-muted-foreground text-foreground'}`}
        >
          {isLLM ? '>' : comment.author_label.slice(0, 1).toUpperCase()}
        </span>
        <span className="font-bold text-foreground">
          {comment.author_label}
        </span>
        {isLLM && <span className="text-muted-foreground">LLM</span>}
        <div className="ml-auto flex items-center gap-2">
          <button
            type="button"
            onClick={() => {
              onToggleResolved(comment)
            }}
            disabled={isResolving}
            className={`border px-1.5 py-0.5 text-2xs disabled:opacity-50 ${comment.resolved ? 'border-status-approved text-status-approved' : 'border-border text-muted-foreground hover:border-primary hover:text-primary'}`}
          >
            {comment.resolved ? '✓ 解決済み' : '未解決'}
          </button>
          <span className="text-muted-foreground">
            {formatRelative(comment.created_at)}
          </span>
        </div>
      </div>
      {comment.anchor_text != null && (
        <blockquote className="border-l-2 border-border pl-2 text-muted-foreground">
          <p className="whitespace-pre-wrap leading-relaxed">
            {comment.anchor_text}
          </p>
        </blockquote>
      )}
      <p
        className={`whitespace-pre-wrap leading-relaxed ${comment.resolved ? 'text-muted-foreground line-through decoration-border' : 'text-foreground'}`}
      >
        {comment.body}
      </p>
    </div>
  )
}
