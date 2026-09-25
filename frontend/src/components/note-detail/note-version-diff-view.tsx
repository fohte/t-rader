import { Columns2, Diff } from 'lucide-react'
import { useState } from 'react'

import { NoteVersionCommentBody } from '#components/note-detail/note-version-comment-body'
import type { NoteVersionComment } from '#lib/api/note-version-types'
import type { NoteVersionDiffRow } from '#lib/note-version-diff'

export type DiffColumnMode = 'one-column' | 'two-column'
export type DiffAnchorSide = 'old' | 'new'

export interface DiffCommentAnchor {
  side: DiffAnchorSide
  lineNumber: number
  text: string
}

export interface NoteVersionDiffViewProps {
  rows: NoteVersionDiffRow[]
  comments: NoteVersionComment[]
  mode: DiffColumnMode
  activeCommentKey: string | null
  replyingCommentId: string | null
  isCreating: boolean
  isReplying: boolean
  resolvingCommentId: string | null
  hasCreateError: boolean
  hasReplyError: boolean
  isCommentsPending: boolean
  hasCommentsError: boolean
  onModeChange: (mode: DiffColumnMode) => void
  onStartComment: (anchor: DiffCommentAnchor) => void
  onCancelComment: () => void
  onCreateComment: (anchor: DiffCommentAnchor, body: string) => void
  onStartReply: (commentId: string) => void
  onCancelReply: () => void
  onReply: (commentId: string, body: string) => void
  onToggleResolved: (comment: NoteVersionComment) => void
}

export function NoteVersionDiffView({
  rows,
  comments,
  mode,
  activeCommentKey,
  replyingCommentId,
  isCreating,
  isReplying,
  resolvingCommentId,
  hasCreateError,
  hasReplyError,
  isCommentsPending,
  hasCommentsError,
  onModeChange,
  onStartComment,
  onCancelComment,
  onCreateComment,
  onStartReply,
  onCancelReply,
  onReply,
  onToggleResolved,
}: NoteVersionDiffViewProps) {
  const unanchoredTopLevel = comments.filter(
    (comment) => comment.parent_id == null && comment.start_line == null,
  )
  const unanchoredComments = commentsForThreads(comments, unanchoredTopLevel)

  return (
    <section className="border border-border bg-card">
      <header className="flex flex-wrap items-center justify-between gap-2 border-b border-border px-3.5 py-2">
        <h2 className="font-mono text-xs font-bold uppercase tracking-wider text-foreground">
          バージョン差分
        </h2>
        <div role="group" aria-label="差分表示" className="flex gap-1">
          <button
            type="button"
            aria-label="統合差分"
            title="統合差分"
            aria-pressed={mode === 'one-column'}
            onClick={() => {
              onModeChange('one-column')
            }}
            className={`inline-flex size-7 items-center justify-center border ${mode === 'one-column' ? 'border-primary bg-primary/10 text-primary' : 'border-border text-muted-foreground hover:text-foreground'}`}
          >
            <Diff className="size-4" aria-hidden="true" />
          </button>
          <button
            type="button"
            aria-label="分割差分"
            title="分割差分"
            aria-pressed={mode === 'two-column'}
            onClick={() => {
              onModeChange('two-column')
            }}
            className={`inline-flex size-7 items-center justify-center border ${mode === 'two-column' ? 'border-primary bg-primary/10 text-primary' : 'border-border text-muted-foreground hover:text-foreground'}`}
          >
            <Columns2 className="size-4" aria-hidden="true" />
          </button>
        </div>
      </header>
      {isCommentsPending && (
        <p className="border-b border-border px-3.5 py-2 font-mono text-2xs text-muted-foreground">
          コメントを読み込み中…
        </p>
      )}
      {hasCommentsError && (
        <p className="border-b border-border px-3.5 py-2 font-mono text-2xs text-primary">
          コメントを読み込めませんでした
        </p>
      )}
      {unanchoredTopLevel.length > 0 && (
        <div className="border-b border-border px-3.5 py-3">
          <h3 className="mb-2 font-mono text-2xs font-bold uppercase tracking-wider text-muted-foreground">
            本文全体のコメント
          </h3>
          <LineCommentGroup
            comments={unanchoredComments}
            anchor={null}
            anchorKey={null}
            allowNewComment={false}
            activeCommentKey={activeCommentKey}
            replyingCommentId={replyingCommentId}
            isCreating={isCreating}
            isReplying={isReplying}
            resolvingCommentId={resolvingCommentId}
            hasCreateError={hasCreateError}
            hasReplyError={hasReplyError}
            onStartComment={onStartComment}
            onCancelComment={onCancelComment}
            onCreateComment={onCreateComment}
            onStartReply={onStartReply}
            onCancelReply={onCancelReply}
            onReply={onReply}
            onToggleResolved={onToggleResolved}
          />
        </div>
      )}
      {rows.length === 0 ? (
        <p className="px-3.5 py-3 font-mono text-xs text-muted-foreground">
          比較する本文はありません。
        </p>
      ) : (
        <div className="divide-y divide-border overflow-x-auto font-mono text-xs">
          {rows.map((row) => (
            <DiffRow
              key={row.key}
              row={row}
              comments={comments}
              mode={mode}
              activeCommentKey={activeCommentKey}
              replyingCommentId={replyingCommentId}
              isCreating={isCreating}
              isReplying={isReplying}
              resolvingCommentId={resolvingCommentId}
              hasCreateError={hasCreateError}
              hasReplyError={hasReplyError}
              onStartComment={onStartComment}
              onCancelComment={onCancelComment}
              onCreateComment={onCreateComment}
              onStartReply={onStartReply}
              onCancelReply={onCancelReply}
              onReply={onReply}
              onToggleResolved={onToggleResolved}
            />
          ))}
        </div>
      )}
    </section>
  )
}

interface DiffRowProps extends Omit<
  NoteVersionDiffViewProps,
  'rows' | 'onModeChange' | 'isCommentsPending' | 'hasCommentsError'
> {
  row: NoteVersionDiffRow
  mode: DiffColumnMode
}

function DiffRow({ row, mode, comments, ...props }: DiffRowProps) {
  if (mode === 'two-column') {
    return (
      <div className="grid min-w-176 grid-cols-2 divide-x divide-border">
        <DiffCell
          side="old"
          lineNumber={row.oldLine}
          text={row.oldText}
          kind={row.kind}
          comments={comments}
          {...props}
        />
        <DiffCell
          side="new"
          lineNumber={row.newLine}
          text={row.newText}
          kind={row.kind}
          comments={comments}
          {...props}
        />
      </div>
    )
  }

  if (row.kind === 'unchanged') {
    return (
      <DiffCell
        side="new"
        lineNumber={row.newLine}
        text={row.newText}
        kind={row.kind}
        comments={comments}
        alsoShowOldComments
        oldLineNumber={row.oldLine}
        {...props}
      />
    )
  }

  return (
    <div>
      {row.oldText != null && (
        <DiffCell
          side="old"
          lineNumber={row.oldLine}
          text={row.oldText}
          kind={row.kind}
          comments={comments}
          {...props}
        />
      )}
      {row.newText != null && (
        <DiffCell
          side="new"
          lineNumber={row.newLine}
          text={row.newText}
          kind={row.kind}
          comments={comments}
          {...props}
        />
      )}
    </div>
  )
}

interface DiffCellProps extends Omit<DiffRowProps, 'row' | 'mode'> {
  side: DiffAnchorSide
  lineNumber: number | null
  text: string | null
  kind: NoteVersionDiffRow['kind']
  oldLineNumber?: number | null
  alsoShowOldComments?: boolean
}

function DiffCell({
  side,
  lineNumber,
  text,
  kind,
  comments,
  oldLineNumber,
  alsoShowOldComments = false,
  ...props
}: DiffCellProps) {
  if (lineNumber == null || text == null) {
    return <div className="min-h-8 bg-surface-strong/40" aria-hidden="true" />
  }

  const changeColor =
    kind === 'removed'
      ? 'border-l-status-rejected bg-status-rejected/10'
      : kind === 'added'
        ? 'border-l-status-approved bg-status-approved/10'
        : kind === 'changed' && side === 'old'
          ? 'border-l-status-rejected bg-status-rejected/10'
          : kind === 'changed'
            ? 'border-l-status-approved bg-status-approved/10'
            : 'border-l-transparent'
  const relevantTopLevel = comments.filter(
    (comment) =>
      comment.parent_id == null &&
      comment.start_line === lineNumber &&
      (comment.anchor_side ?? 'new') === side,
  )
  const relevantComments = commentsForThreads(comments, relevantTopLevel)
  const oldTopLevel = alsoShowOldComments
    ? comments.filter(
        (comment) =>
          comment.parent_id == null &&
          comment.start_line === oldLineNumber &&
          (comment.anchor_side ?? 'new') === 'old',
      )
    : []
  const oldComments = alsoShowOldComments
    ? commentsForThreads(comments, oldTopLevel)
    : []
  const anchor: DiffCommentAnchor = { side, lineNumber, text }
  const anchorKey = `${side}:${String(lineNumber)}`

  return (
    <div className={`border-l-2 ${changeColor}`}>
      <div className="flex min-h-8 items-start gap-2 px-2 py-1">
        <span className="w-8 shrink-0 select-none text-right text-muted-foreground">
          {String(lineNumber)}
        </span>
        <pre className="min-w-0 flex-1 whitespace-pre-wrap break-words text-foreground">
          {text === '' ? ' ' : text}
        </pre>
      </div>
      {alsoShowOldComments &&
        oldLineNumber != null &&
        oldComments.length > 0 && (
          <div className="pl-10">
            <LineCommentGroup
              comments={oldComments}
              anchor={{ side: 'old', lineNumber: oldLineNumber, text }}
              anchorKey={`old:${String(oldLineNumber)}`}
              allowNewComment={false}
              {...props}
            />
          </div>
        )}
      <div className="pl-10">
        <LineCommentGroup
          comments={relevantComments}
          anchor={anchor}
          anchorKey={anchorKey}
          {...props}
        />
      </div>
    </div>
  )
}

interface LineCommentGroupProps extends Omit<DiffRowProps, 'row' | 'mode'> {
  comments: NoteVersionComment[]
  anchor: DiffCommentAnchor | null
  anchorKey: string | null
  allowNewComment?: boolean
}

function LineCommentGroup({
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

function commentsForThreads(
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
