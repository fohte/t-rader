import { Children, isValidElement, type ReactNode, useContext } from 'react'
import { type ExtraProps } from 'react-markdown'

import {
  MarkdownBodyRangeCommentContext,
  type MarkdownBodyRangeCommentContextValue,
  type MarkdownBodyRangeCommentProps,
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

function descendantSourceBlockRanges(content: ReactNode): SourceLineRange[] {
  const ranges: SourceLineRange[] = []
  const visit = (node: ReactNode): void => {
    for (const child of Children.toArray(node)) {
      if (!isValidElement<MarkdownBodyRangeBlockProps>(child)) continue
      if (child.type === MarkdownBodyRangeBlock) {
        const childProps = child.props
        const range = sourceLineRange(childProps.node)
        if (range != null) ranges.push(range)
        visit(childProps.element)
        visit(childProps.children)
        continue
      }
      visit(child.props.children)
    }
  }
  visit(content)
  return ranges
}

function commentBlockPresentation(
  range: SourceLineRange,
  context: MarkdownBodyRangeCommentContextValue,
  descendantRanges: SourceLineRange[],
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
  const belongsToDescendant = (candidate: SourceLineRange): boolean =>
    descendantRanges.some(
      (descendant) =>
        descendant.startLine <= candidate.endLine &&
        descendant.endLine >= candidate.endLine,
    )
  const threadTopLevelComments = anchoredComments.filter((comment) => {
    const commentRange = rangeForComment(comment)
    return (
      commentRange.endLine >= range.startLine &&
      commentRange.endLine <= range.endLine &&
      !belongsToDescendant(commentRange)
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
    activeRange.endLine <= range.endLine &&
    !belongsToDescendant(activeRange)
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

interface RangeBlockGutterButtonProps {
  range: SourceLineRange
  isSelected: boolean
  onPointerDown: (range: SourceLineRange) => void
  onClick: (range: SourceLineRange, shiftKey: boolean) => void
  className: string
}

function RangeBlockGutterButton({
  range,
  isSelected,
  onPointerDown,
  onClick,
  className,
}: RangeBlockGutterButtonProps) {
  return (
    <button
      type="button"
      aria-label={`行 ${String(range.startLine)}${range.endLine > range.startLine ? ` から ${String(range.endLine)}` : ''} にコメント`}
      onPointerDown={() => {
        onPointerDown(range)
      }}
      onClick={(event) => {
        onClick(range, event.shiftKey)
      }}
      className={className}
    >
      {isSelected ? '+' : String(range.startLine)}
    </button>
  )
}

interface RangeBlockCommentThreadProps {
  presentation: CommentBlockPresentation
  props: MarkdownBodyRangeCommentProps
}

function RangeBlockCommentThread({
  presentation,
  props,
}: RangeBlockCommentThreadProps) {
  return (
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
  )
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

  const childRanges = descendantSourceBlockRanges(content)
  const presentation = commentBlockPresentation(range, context, childRanges)
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
              <RangeBlockGutterButton
                range={range}
                isSelected={presentation.isSelected}
                onPointerDown={context.onGutterPointerDown}
                onClick={context.onGutterClick}
                className={`min-h-7 w-full ${presentation.isSelected || presentation.isCommented ? 'text-primary' : 'text-muted-foreground hover:text-primary'}`}
              />
            </td>
            {content}
          </tr>
          {showThread && (
            <tr>
              <td colSpan={cells} className="px-3">
                <RangeBlockCommentThread
                  presentation={presentation}
                  props={props}
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
        <div className="min-w-0">
          <div
            data-source-block="true"
            data-source-start-line={range.startLine}
            data-source-end-line={range.endLine}
          >
            {content}
          </div>
          {showThread && (
            <RangeBlockCommentThread
              presentation={presentation}
              props={props}
            />
          )}
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
          <RangeBlockGutterButton
            range={range}
            isSelected={presentation.isSelected}
            onPointerDown={context.onGutterPointerDown}
            onClick={context.onGutterClick}
            className={`min-h-7 w-11 shrink-0 select-none px-1 text-right font-mono text-2xs ${presentation.isSelected || presentation.isCommented ? 'text-primary' : 'text-muted-foreground hover:text-primary'}`}
          />
          <div className="min-w-0 flex-1">{content}</div>
        </div>
        {showThread && (
          <div className="ml-11">
            <RangeBlockCommentThread
              presentation={presentation}
              props={props}
            />
          </div>
        )}
      </div>
    </MarkdownBodySourceBlockContext.Provider>
  )
}
