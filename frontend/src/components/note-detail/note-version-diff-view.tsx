import { SegmentedControl } from '@fohte/ui/segmented-control'

import {
  commentsForThreads,
  type DiffCommentAnchor,
  getDiffCommentAnchorKey,
  LineCommentGroup,
} from '#components/note-detail/note-version-line-comment-group'
import type { NoteVersionComment } from '#lib/api/note-version-types'
import type { NoteVersionDiffRow } from '#lib/note-version-diff'
import { cn } from '#lib/utils'

export type DiffColumnMode = 'one-column' | 'two-column'

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
        <SegmentedControl
          aria-label="差分表示"
          value={mode}
          options={[
            { value: 'one-column', label: '統合' },
            { value: 'two-column', label: '分割' },
          ]}
          onValueChange={onModeChange}
        />
      </header>
      {isCommentsPending && (
        <p className="border-b border-border px-3.5 py-2 font-mono text-2xs text-muted-foreground">
          コメントを読み込み中…
        </p>
      )}
      {hasCommentsError && (
        <p className="border-b border-border px-3.5 py-2 font-mono text-2xs text-destructive">
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
  side: DiffCommentAnchor['side']
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
    kind === 'changed'
      ? side === 'old'
        ? 'border-l-status-rejected bg-status-rejected/10'
        : 'border-l-status-approved bg-status-approved/10'
      : {
          added: 'border-l-status-approved bg-status-approved/10',
          removed: 'border-l-status-rejected bg-status-rejected/10',
          unchanged: 'border-l-transparent',
        }[kind]
  const relevantTopLevel = commentsCoveringLine(comments, side, lineNumber)
  const relevantAtEnd = relevantTopLevel.filter(
    (comment) => (comment.end_line ?? comment.start_line) === lineNumber,
  )
  const relevantComments = commentsForThreads(comments, relevantAtEnd)
  const oldTopLevel =
    alsoShowOldComments && oldLineNumber != null
      ? commentsCoveringLine(comments, 'old', oldLineNumber)
      : []
  const oldAtEnd = oldTopLevel.filter(
    (comment) => (comment.end_line ?? comment.start_line) === oldLineNumber,
  )
  const oldComments = commentsForThreads(comments, oldAtEnd)
  const hasCommentRange =
    hasMultiLineRange(relevantTopLevel) || hasMultiLineRange(oldTopLevel)
  const anchor: DiffCommentAnchor = { side, lineNumber, text }
  const anchorKey = getDiffCommentAnchorKey(anchor)

  return (
    <div
      className={cn(
        'border-l-2',
        changeColor,
        hasCommentRange && 'ring-1 ring-inset ring-primary/50',
      )}
    >
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

function commentsCoveringLine(
  comments: NoteVersionComment[],
  side: DiffCommentAnchor['side'],
  lineNumber: number,
): NoteVersionComment[] {
  return comments.filter((comment) => {
    if (comment.parent_id != null || comment.start_line == null) return false
    const endLine = comment.end_line ?? comment.start_line
    return (
      comment.start_line <= lineNumber &&
      lineNumber <= endLine &&
      (comment.anchor_side ?? 'new') === side
    )
  })
}

function hasMultiLineRange(comments: NoteVersionComment[]): boolean {
  return comments.some(
    (comment) =>
      comment.start_line != null &&
      comment.end_line != null &&
      comment.start_line < comment.end_line,
  )
}
