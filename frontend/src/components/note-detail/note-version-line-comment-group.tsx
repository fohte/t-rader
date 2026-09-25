import { useState } from 'react'

import { NoteVersionCommentBody } from '#components/note-detail/note-version-comment-body'
import type { NoteVersionComment } from '#lib/api/note-version-types'

type DiffAnchorSide = 'old' | 'new'

export interface DiffCommentAnchor {
  side: DiffAnchorSide
  lineNumber: number
  text: string
}

interface LineCommentGroupProps {
  comments: NoteVersionComment[]
  anchor: DiffCommentAnchor | null
  anchorKey: string | null
  allowNewComment?: boolean
  activeCommentKey: string | null
  replyingCommentId: string | null
  isCreating: boolean
  isReplying: boolean
  resolvingCommentId: string | null
  hasCreateError: boolean
  hasReplyError: boolean
  onStartComment: (anchor: DiffCommentAnchor) => void
  onCancelComment: () => void
  onCreateComment: (anchor: DiffCommentAnchor, body: string) => void
  onStartReply: (commentId: string) => void
  onCancelReply: () => void
  onReply: (commentId: string, body: string) => void
  onToggleResolved: (comment: NoteVersionComment) => void
}

export function LineCommentGroup({
  comments,
  anchor,
  anchorKey,
  allowNewComment = true,
  activeCommentKey,
  replyingCommentId,
  isCreating,
  isReplying,
  resolvingCommentId,
  hasCreateError,
  hasReplyError,
  onStartComment,
  onCancelComment,
  onCreateComment,
  onStartReply,
  onCancelReply,
  onReply,
  onToggleResolved,
}: LineCommentGroupProps) {
  const [commentDraft, setCommentDraft] = useState('')
  const [replyDraft, setReplyDraft] = useState('')
  const topLevel = comments.filter((comment) => comment.parent_id == null)
  const repliesByParent = new Map<string, NoteVersionComment[]>()
  for (const comment of comments) {
    if (comment.parent_id != null) {
      const replies = repliesByParent.get(comment.parent_id) ?? []
      replies.push(comment)
      repliesByParent.set(comment.parent_id, replies)
    }
  }

  const submit = (): void => {
    const body = commentDraft.trim()
    if (body === '' || isCreating || anchor == null) return
    onCreateComment(anchor, body)
  }

  return (
    <div className="space-y-2 border-l border-border py-2 pl-3 pr-2 text-xs">
      {topLevel.map((comment) => (
        <div key={comment.id} className="space-y-2">
          <NoteVersionCommentBody
            comment={comment}
            onToggleResolved={onToggleResolved}
            isResolving={resolvingCommentId === comment.id}
          />
          {(repliesByParent.get(comment.id) ?? []).map((reply) => (
            <div key={reply.id} className="border-l border-border pl-3">
              <NoteVersionCommentBody
                comment={reply}
                onToggleResolved={onToggleResolved}
                isResolving={resolvingCommentId === reply.id}
              />
            </div>
          ))}
          {replyingCommentId === comment.id ? (
            <div className="flex flex-wrap gap-2">
              <input
                autoFocus
                aria-label="返信"
                value={replyDraft}
                onChange={(event) => {
                  setReplyDraft(event.target.value)
                }}
                className="min-w-0 flex-1 border border-border bg-background px-2 py-1 font-mono text-2xs text-foreground outline-none focus:border-muted-foreground"
                placeholder="返信を入力"
              />
              <button
                type="button"
                disabled={replyDraft.trim() === '' || isReplying}
                onClick={() => {
                  const body = replyDraft.trim()
                  if (body === '') return
                  onReply(comment.id, body)
                }}
                className="border border-border px-2 py-1 font-mono text-2xs text-foreground hover:border-primary hover:text-primary disabled:opacity-50"
              >
                送信
              </button>
              <button
                type="button"
                onClick={() => {
                  setReplyDraft('')
                  onCancelReply()
                }}
                className="px-2 py-1 font-mono text-2xs text-muted-foreground hover:text-foreground"
              >
                キャンセル
              </button>
            </div>
          ) : (
            <button
              type="button"
              onClick={() => {
                setReplyDraft('')
                onStartReply(comment.id)
              }}
              className="font-mono text-2xs text-muted-foreground hover:text-primary"
            >
              返信
            </button>
          )}
          {hasReplyError && replyingCommentId === comment.id && (
            <p className="font-mono text-2xs text-primary">
              返信に失敗しました
            </p>
          )}
        </div>
      ))}
      {anchorKey !== null && activeCommentKey === anchorKey ? (
        <div className="space-y-2">
          <textarea
            aria-label="行コメント"
            value={commentDraft}
            onChange={(event) => {
              setCommentDraft(event.target.value)
            }}
            rows={2}
            className="w-full border border-border bg-background px-2 py-1.5 font-mono text-2xs text-foreground outline-none focus:border-muted-foreground"
            placeholder="この行へのコメント"
          />
          <div className="flex gap-2">
            <button
              type="button"
              disabled={commentDraft.trim() === '' || isCreating}
              onClick={submit}
              className="border border-border px-2 py-1 font-mono text-2xs text-foreground hover:border-primary hover:text-primary disabled:opacity-50"
            >
              送信
            </button>
            <button
              type="button"
              onClick={() => {
                setCommentDraft('')
                onCancelComment()
              }}
              className="px-2 py-1 font-mono text-2xs text-muted-foreground hover:text-foreground"
            >
              キャンセル
            </button>
          </div>
          {hasCreateError && (
            <p className="font-mono text-2xs text-primary">
              コメントを送信できませんでした
            </p>
          )}
        </div>
      ) : anchor !== null && allowNewComment ? (
        <button
          type="button"
          onClick={() => {
            setCommentDraft('')
            onStartComment(anchor)
          }}
          className="font-mono text-2xs text-muted-foreground hover:text-primary"
        >
          + 行コメント
        </button>
      ) : null}
    </div>
  )
}

export function commentsForThreads(
  comments: NoteVersionComment[],
  topLevelComments: NoteVersionComment[],
): NoteVersionComment[] {
  const parentIds = new Set(topLevelComments.map((comment) => comment.id))
  return comments.filter(
    (comment) =>
      (comment.parent_id == null && parentIds.has(comment.id)) ||
      (comment.parent_id != null && parentIds.has(comment.parent_id)),
  )
}
