import { Button } from '@fohte/ui/button'
import { Input } from '@fohte/ui/input'
import { Textarea } from '@fohte/ui/textarea'
import { useState } from 'react'

import { NoteVersionCommentBody } from '#components/note-detail/note-version-comment-body'
import type { NoteVersionComment } from '#lib/api/note-version-types'

type DiffAnchorSide = 'old' | 'new'

export interface DiffCommentAnchor {
  side: DiffAnchorSide
  lineNumber: number
  endLineNumber?: number
  text: string
  isTextSelection?: boolean
  textOffset?: number
}

export function getDiffCommentAnchorKey(anchor: DiffCommentAnchor): string {
  const endLine = anchor.endLineNumber ?? anchor.lineNumber
  return endLine === anchor.lineNumber
    ? `${anchor.side}:${String(anchor.lineNumber)}`
    : `${anchor.side}:${String(anchor.lineNumber)}-${String(endLine)}`
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

  const isTextSelection = anchor?.isTextSelection === true
  const composerLabel = isTextSelection
    ? '選択した箇所へのコメント'
    : '行コメント'
  const composerPlaceholder = isTextSelection
    ? '選択した箇所へのコメント'
    : 'この行へのコメント'

  return (
    <div
      data-comment-text-ignore="true"
      className="space-y-2 border-l border-border py-2 pl-3 pr-2 text-xs"
    >
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
              <Input
                autoFocus
                aria-label="返信"
                value={replyDraft}
                onChange={(event) => {
                  setReplyDraft(event.target.value)
                }}
                className="flex-1"
                placeholder="返信を入力"
              />
              <Button
                type="button"
                variant="outline"
                size="sm"
                disabled={replyDraft.trim() === '' || isReplying}
                onClick={() => {
                  const body = replyDraft.trim()
                  if (body === '') return
                  onReply(comment.id, body)
                }}
              >
                {isReplying ? '送信中…' : '送信'}
              </Button>
              <Button
                type="button"
                variant="plain"
                onClick={() => {
                  setReplyDraft('')
                  onCancelReply()
                }}
              >
                キャンセル
              </Button>
            </div>
          ) : (
            <Button
              type="button"
              variant="plain"
              onClick={() => {
                setReplyDraft('')
                onStartReply(comment.id)
              }}
            >
              返信
            </Button>
          )}
          {hasReplyError && replyingCommentId === comment.id && (
            <p className="font-mono text-2xs text-destructive">
              返信に失敗しました
            </p>
          )}
        </div>
      ))}
      {anchorKey !== null && activeCommentKey === anchorKey ? (
        <div className="space-y-2">
          <Textarea
            aria-label={composerLabel}
            value={commentDraft}
            onChange={(event) => {
              setCommentDraft(event.target.value)
            }}
            placeholder={composerPlaceholder}
          />
          <div className="flex gap-2">
            <Button
              type="button"
              variant="outline"
              size="sm"
              disabled={commentDraft.trim() === '' || isCreating}
              onClick={submit}
            >
              {isCreating ? '送信中…' : '送信'}
            </Button>
            <Button
              type="button"
              variant="plain"
              onClick={() => {
                setCommentDraft('')
                onCancelComment()
              }}
            >
              キャンセル
            </Button>
          </div>
          {hasCreateError && (
            <p className="font-mono text-2xs text-destructive">
              コメントを送信できませんでした
            </p>
          )}
        </div>
      ) : anchor !== null && allowNewComment ? (
        <Button
          type="button"
          variant="plain"
          onClick={() => {
            setCommentDraft('')
            onStartComment(anchor)
          }}
        >
          + 行コメント
        </Button>
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
