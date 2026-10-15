import { Children, type ReactNode, useContext } from 'react'
import { type ExtraProps } from 'react-markdown'

import {
  MarkdownBodyRangeCommentContext,
  type MarkdownBodyRangeCommentContextValue,
  MarkdownBodySourceBlockContext,
} from '#components/note-detail/markdown-body-range-comment-context'
import {
  commentsForThreads,
  type DiffCommentAnchor,
  getDiffCommentAnchorKey,
  LineCommentGroup,
} from '#components/note-detail/note-version-line-comment-group'
import type { SourceLineRange } from '#components/note-detail/use-markdown-body-comment-selection'
import type { NoteVersionComment } from '#lib/api/note-version-types'

type MarkdownNode = NonNullable<ExtraProps['node']>

interface CommentBlockPresentation {
  anchor: DiffCommentAnchor
  anchorKey: string | null
  activeCommentKey: string | null
  isActive: boolean
  isCommented: boolean
  isSelected: boolean
  comments: NoteVersionComment[]
}

interface MarkdownBodyRangeBlockProps {
  node?: MarkdownNode
  children?: ReactNode
  element?: ReactNode
  variant?: 'block' | 'table-row'
  showGutter?: boolean
}

function sourceLineRange(
  node: MarkdownNode | undefined,
): SourceLineRange | null {
  if (node?.position == null) return null
  return {
    startLine: node.position.start.line,
    endLine: node.position.end.line,
  }
}

function commentBlockPresentation(
  range: SourceLineRange,
  context: MarkdownBodyRangeCommentContextValue,
): CommentBlockPresentation {
  const { props, selectedRange } = context
  const anchoredComments = props.comments.filter(
    (comment) =>
      comment.parent_id == null &&
      comment.start_line != null &&
      (comment.anchor_side == null || comment.anchor_side === 'new'),
  )
  const rangeForComment = (comment: NoteVersionComment): SourceLineRange => ({
    startLine: comment.start_line ?? range.startLine,
    endLine: comment.end_line ?? comment.start_line ?? range.startLine,
  })
  const overlapsBlock = (candidate: SourceLineRange): boolean =>
    candidate.startLine <= range.endLine && candidate.endLine >= range.startLine
  const threadTopLevelComments = anchoredComments.filter((comment) => {
    const commentRange = rangeForComment(comment)
    return (
      commentRange.endLine >= range.startLine &&
      commentRange.endLine <= range.endLine
    )
  })
  const activeAnchor = props.activeCommentAnchor
  const activeRange =
    activeAnchor == null
      ? null
      : {
          startLine: activeAnchor.lineNumber,
          endLine: activeAnchor.endLineNumber ?? activeAnchor.lineNumber,
        }
  const isActive =
    activeAnchor != null &&
    activeAnchor.side === 'new' &&
    activeRange != null &&
    activeRange.endLine >= range.startLine &&
    activeRange.endLine <= range.endLine
  const anchor = isActive
    ? activeAnchor
    : threadTopLevelComments[0] == null
      ? {
          side: 'new' as const,
          lineNumber: range.startLine,
          endLineNumber: range.endLine,
          text: '',
        }
      : {
          side: 'new' as const,
          lineNumber: threadTopLevelComments[0].start_line ?? range.startLine,
          endLineNumber:
            threadTopLevelComments[0].end_line ??
            threadTopLevelComments[0].start_line ??
            range.startLine,
          text: threadTopLevelComments[0].anchor_text ?? '',
        }
  return {
    anchor,
    anchorKey: isActive
      ? props.activeCommentKey
      : getDiffCommentAnchorKey(anchor),
    activeCommentKey: isActive ? props.activeCommentKey : null,
    isActive,
    isCommented: anchoredComments.some((comment) =>
      overlapsBlock(rangeForComment(comment)),
    ),
    isSelected:
      (selectedRange != null && overlapsBlock(selectedRange)) ||
      (activeRange != null && overlapsBlock(activeRange)),
    comments: commentsForThreads(props.comments, threadTopLevelComments),
  }
}

export function MarkdownBodyRangeBlock({
  node,
  children,
  element,
  variant = 'block',
  showGutter = true,
}: MarkdownBodyRangeBlockProps) {
  const context = useContext(MarkdownBodyRangeCommentContext)
  const parentSourceBlock = useContext(MarkdownBodySourceBlockContext)
  const sourceBlockTag = node?.tagName ?? 'block'
  const isNestedSourceBlock =
    parentSourceBlock != null &&
    !(parentSourceBlock === 'li' && sourceBlockTag === 'li')
  const range = sourceLineRange(node)
  const content = element ?? children
  const rowClassName = 'border-b border-border last:border-b-0'
  if (context == null || isNestedSourceBlock || range == null) {
    return variant === 'table-row' ? (
      <tr className={rowClassName}>{content}</tr>
    ) : (
      content
    )
  }

  const presentation = commentBlockPresentation(range, context)
  const { props } = context
  const showThread = presentation.isActive || presentation.comments.length > 0

  if (variant === 'table-row') {
    const cells = Children.count(content) + 1
    return (
      <MarkdownBodySourceBlockContext.Provider value={sourceBlockTag}>
        <>
          <tr
            data-source-block="true"
            data-source-start-line={range.startLine}
            data-source-end-line={range.endLine}
            className={`${rowClassName} ${presentation.isSelected ? 'bg-primary/10' : ''} ${presentation.isCommented ? 'border-l-2 border-primary' : ''}`}
          >
            <td className="w-11 select-none px-1 text-right font-mono text-2xs">
              <button
                type="button"
                aria-label={`行 ${String(range.startLine)}${range.endLine > range.startLine ? ` から ${String(range.endLine)}` : ''} にコメント`}
                onPointerDown={() => {
                  context.onGutterPointerDown(range)
                }}
                onClick={(event) => {
                  context.onGutterClick(range, event.shiftKey)
                }}
                className={`min-h-7 w-full ${presentation.isSelected || presentation.isCommented ? 'text-primary' : 'text-muted-foreground hover:text-primary'}`}
              >
                {presentation.isSelected ? '+' : String(range.startLine)}
              </button>
            </td>
            {content}
          </tr>
          {showThread && (
            <tr>
              <td colSpan={cells} className="px-3">
                <LineCommentGroup
                  comments={presentation.comments}
                  anchor={presentation.anchor}
                  anchorKey={presentation.anchorKey}
                  allowNewComment={false}
                  activeCommentKey={presentation.activeCommentKey}
                  replyingCommentId={props.replyingCommentId}
                  isCreating={props.isCreating}
                  isReplying={props.isReplying}
                  resolvingCommentId={props.resolvingCommentId}
                  hasCreateError={props.hasCreateError}
                  hasReplyError={props.hasReplyError}
                  onStartComment={props.onStartComment}
                  onCancelComment={props.onCancelComment}
                  onCreateComment={props.onCreateComment}
                  onStartReply={props.onStartReply}
                  onCancelReply={props.onCancelReply}
                  onReply={props.onReply}
                  onToggleResolved={props.onToggleResolved}
                />
              </td>
            </tr>
          )}
        </>
      </MarkdownBodySourceBlockContext.Provider>
    )
  }

  if (!showGutter) {
    return (
      <MarkdownBodySourceBlockContext.Provider value={sourceBlockTag}>
        <div
          data-source-block="true"
          data-source-start-line={range.startLine}
          data-source-end-line={range.endLine}
        >
          {content}
        </div>
      </MarkdownBodySourceBlockContext.Provider>
    )
  }

  return (
    <MarkdownBodySourceBlockContext.Provider value={sourceBlockTag}>
      <div
        data-source-block="true"
        data-source-start-line={range.startLine}
        data-source-end-line={range.endLine}
        className="min-w-0"
      >
        <div
          className={`flex items-start ${presentation.isSelected ? 'bg-primary/10' : ''} ${presentation.isCommented ? 'border-l-2 border-primary' : 'border-l-2 border-transparent'}`}
        >
          <button
            type="button"
            aria-label={`行 ${String(range.startLine)}${range.endLine > range.startLine ? ` から ${String(range.endLine)}` : ''} にコメント`}
            onPointerDown={() => {
              context.onGutterPointerDown(range)
            }}
            onClick={(event) => {
              context.onGutterClick(range, event.shiftKey)
            }}
            className={`min-h-7 w-11 shrink-0 select-none px-1 text-right font-mono text-2xs ${presentation.isSelected || presentation.isCommented ? 'text-primary' : 'text-muted-foreground hover:text-primary'}`}
          >
            {presentation.isSelected ? '+' : String(range.startLine)}
          </button>
          <div className="min-w-0 flex-1">{content}</div>
        </div>
        {showThread && (
          <div className="ml-11">
            <LineCommentGroup
              comments={presentation.comments}
              anchor={presentation.anchor}
              anchorKey={presentation.anchorKey}
              allowNewComment={false}
              activeCommentKey={presentation.activeCommentKey}
              replyingCommentId={props.replyingCommentId}
              isCreating={props.isCreating}
              isReplying={props.isReplying}
              resolvingCommentId={props.resolvingCommentId}
              hasCreateError={props.hasCreateError}
              hasReplyError={props.hasReplyError}
              onStartComment={props.onStartComment}
              onCancelComment={props.onCancelComment}
              onCreateComment={props.onCreateComment}
              onStartReply={props.onStartReply}
              onCancelReply={props.onCancelReply}
              onReply={props.onReply}
              onToggleResolved={props.onToggleResolved}
            />
          </div>
        )}
      </div>
    </MarkdownBodySourceBlockContext.Provider>
  )
}
