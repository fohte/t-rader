import {
  Children,
  cloneElement,
  type CSSProperties,
  isValidElement,
  type ReactNode,
  useContext,
} from 'react'
import { type ExtraProps } from 'react-markdown'

import {
  MarkdownBodyRangeCommentContext,
  type MarkdownBodyRangeCommentContextValue,
  type MarkdownBodyRangeCommentProps,
  MarkdownBodySourceBlockContext,
  MarkdownBodyTableCellGutterContext,
  NOTE_BODY_GUTTER_GAP_REM,
  NOTE_BODY_INDENT_REM,
} from '#components/note-detail/markdown-body-range-comment-context'
import { highlightFirstTextMatch } from '#components/note-detail/markdown-body-text-highlight'
import { NOTE_BODY_GUTTER_CARD_PADDING_REM } from '#components/note-detail/note-body-card'
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
  isTextSelection: boolean
  highlightText: string | null
  comments: NoteVersionComment[]
}

interface MarkdownBodyRangeBlockProps {
  node?: MarkdownNode
  children?: ReactNode
  element?: ReactNode
  variant?: 'block' | 'table-row'
  showGutter?: boolean
  listDepth?: number
}

interface RangeBlockElementProps {
  className?: string
  children?: ReactNode
  [key: `data-${string}`]: string | number | boolean | undefined
}

interface MarkdownNodeElementProps {
  node?: MarkdownNode
  children?: ReactNode
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

function descendantSourceBlockRanges(
  content: ReactNode,
  isInsideListItem: boolean,
): SourceLineRange[] {
  const ranges: SourceLineRange[] = []
  const visit = (node: ReactNode, isInsideList: boolean): void => {
    for (const child of Children.toArray(node)) {
      if (!isValidElement<MarkdownBodyRangeBlockProps>(child)) continue
      if (child.type === MarkdownBodyRangeBlock) {
        const childProps = child.props
        const isThreadOwnerListItem =
          childProps.node?.tagName === 'li' && isInsideList
        const range = isThreadOwnerListItem
          ? sourceLineRange(childProps.node)
          : null
        if (range != null) ranges.push(range)
        visit(childProps.element, isThreadOwnerListItem)
        visit(childProps.children, isThreadOwnerListItem)
        continue
      }
      const childProps = child.props as MarkdownNodeElementProps
      const tagName = childProps.node?.tagName
      const isThreadOwnerListItem = tagName === 'li' && isInsideList
      if (isThreadOwnerListItem) {
        const range = sourceLineRange(childProps.node)
        if (range != null) ranges.push(range)
      }
      visit(
        childProps.children,
        tagName === 'li'
          ? isThreadOwnerListItem
          : tagName == null || tagName === 'ul' || tagName === 'ol'
            ? isInsideList
            : false,
      )
    }
  }
  visit(content, isInsideListItem)
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
  const isTextSelection = isActive && activeAnchor.isTextSelection === true
  const commentsOverlappingBlock = anchoredComments.filter((comment) =>
    overlapsBlock(rangeForComment(comment)),
  )
  const commentsForTextHighlight = commentsOverlappingBlock.filter(
    (comment) => !belongsToDescendant(rangeForComment(comment)),
  )
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
    isCommented: commentsOverlappingBlock.length > 0,
    isTextSelection,
    highlightText: isTextSelection
      ? activeAnchor.text
      : isActive
        ? null
        : (commentsForTextHighlight.find(
            (comment) =>
              comment.anchor_text != null && comment.anchor_text !== '',
          )?.anchor_text ?? null),
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
  style?: CSSProperties
}

function RangeBlockGutterButton({
  range,
  isSelected,
  onPointerDown,
  onClick,
  className,
  style,
}: RangeBlockGutterButtonProps) {
  return (
    <button
      type="button"
      aria-label={`行 ${String(range.startLine)}${range.endLine > range.startLine ? ` から ${String(range.endLine)}` : ''} にコメント`}
      data-comment-text-ignore="true"
      onPointerDown={() => {
        onPointerDown(range)
      }}
      onClick={(event) => {
        onClick(range, event.shiftKey)
      }}
      className={className}
      style={style}
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
  listDepth = 0,
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

  const childRanges = descendantSourceBlockRanges(
    content,
    sourceBlockTag === 'li',
  )
  const presentation = commentBlockPresentation(range, context, childRanges)
  const textHighlight =
    presentation.highlightText == null || presentation.highlightText === ''
      ? null
      : highlightFirstTextMatch(content, presentation.highlightText, {
          textOffset: presentation.isTextSelection
            ? presentation.anchor.textOffset
            : undefined,
          refChipTexts: context.refChipTexts,
        })
  const isTextHighlightPartial = textHighlight?.match === 'partial'
  const isBlockSelected =
    (presentation.isSelected &&
      (!presentation.isTextSelection || !isTextHighlightPartial)) ||
    (textHighlight != null && textHighlight.match !== 'partial')
  const renderedContent = textHighlight?.children ?? content
  const { props } = context
  const showThread = presentation.isActive || presentation.comments.length > 0
  const gutterButtonClassName = `z-10 min-h-7 w-11 select-none text-right font-mono text-2xs ${presentation.isSelected || presentation.isCommented ? 'text-primary' : 'text-muted-foreground hover:text-primary'}`
  const commentMarker =
    presentation.isSelected || presentation.isCommented ? (
      <span
        aria-hidden="true"
        data-comment-text-ignore="true"
        className="pointer-events-none absolute inset-y-0 w-0.5 bg-primary"
        style={{
          left:
            sourceBlockTag === 'li'
              ? `calc(-${String(NOTE_BODY_GUTTER_CARD_PADDING_REM)}rem - ${String(listDepth * NOTE_BODY_INDENT_REM)}rem)`
              : `-${String(NOTE_BODY_GUTTER_CARD_PADDING_REM)}rem`,
        }}
      />
    ) : null
  const commentThread = showThread ? (
    <RangeBlockCommentThread presentation={presentation} props={props} />
  ) : null
  const indentedCommentThread =
    commentThread == null ? null : <div className="ml-11">{commentThread}</div>

  if (variant === 'table-row') {
    const cells = Children.count(content)
    const rowCells = Children.toArray(renderedContent)
    const tableCellGutter = (
      <>
        <RangeBlockGutterButton
          range={range}
          isSelected={presentation.isSelected}
          onPointerDown={context.onGutterPointerDown}
          onClick={context.onGutterClick}
          className={`absolute -left-11 top-1.5 ${gutterButtonClassName}`}
          style={{ paddingRight: `${String(NOTE_BODY_GUTTER_GAP_REM)}rem` }}
        />
        {commentMarker}
      </>
    )
    if (rowCells[0] != null) {
      rowCells[0] = (
        <MarkdownBodyTableCellGutterContext.Provider
          key="gutter"
          value={tableCellGutter}
        >
          {rowCells[0]}
        </MarkdownBodyTableCellGutterContext.Provider>
      )
    }
    return (
      <MarkdownBodySourceBlockContext.Provider value={sourceBlockTag}>
        <>
          <tr
            data-source-block="true"
            data-source-start-line={range.startLine}
            data-source-end-line={range.endLine}
            className={`${rowClassName} ${isBlockSelected ? 'bg-primary/10' : ''}`}
          >
            {rowCells}
          </tr>
          {showThread && (
            <tr>
              <td colSpan={cells} className="px-3">
                {commentThread}
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
        <div className={`min-w-0 ${isBlockSelected ? 'bg-primary/10' : ''}`}>
          <div
            data-source-block="true"
            data-source-start-line={range.startLine}
            data-source-end-line={range.endLine}
          >
            {renderedContent}
          </div>
          {commentThread}
        </div>
      </MarkdownBodySourceBlockContext.Provider>
    )
  }

  const gutterStyle: CSSProperties = {
    ...(sourceBlockTag === 'li'
      ? {
          left: `calc(-2.75rem - ${String(listDepth * NOTE_BODY_INDENT_REM)}rem)`,
          top: 0,
        }
      : sourceBlockTag === 'pre'
        ? { left: '-2.75rem', top: '0.75rem' }
        : sourceBlockTag === 'blockquote'
          ? { left: '-2.75rem', top: '0.375rem' }
          : { left: '-2.75rem' }),
    paddingRight: `${String(NOTE_BODY_GUTTER_GAP_REM)}rem`,
  }
  const gutterButton = (
    <RangeBlockGutterButton
      range={range}
      isSelected={presentation.isSelected}
      onPointerDown={context.onGutterPointerDown}
      onClick={context.onGutterClick}
      className={`absolute ${gutterButtonClassName}`}
      style={gutterStyle}
    />
  )

  const isIntrinsicElement =
    isValidElement<RangeBlockElementProps>(renderedContent) &&
    typeof renderedContent.type === 'string'
  if (isIntrinsicElement && sourceBlockTag === 'li') {
    const hostElement = cloneElement(renderedContent, {
      className: `${renderedContent.props.className ?? ''} relative ${isBlockSelected ? 'bg-primary/10' : ''}`,
      'data-source-block': 'true',
      'data-source-start-line': range.startLine,
      'data-source-end-line': range.endLine,
      children: (
        <>
          {renderedContent.props.children}
          {gutterButton}
          {commentMarker}
          {indentedCommentThread}
        </>
      ),
    })
    return (
      <MarkdownBodySourceBlockContext.Provider value={sourceBlockTag}>
        {hostElement}
      </MarkdownBodySourceBlockContext.Provider>
    )
  }

  const hostElement = isIntrinsicElement
    ? cloneElement(renderedContent, {
        className: `${renderedContent.props.className ?? ''} ${isBlockSelected ? 'bg-primary/10' : ''}`,
      })
    : renderedContent

  return (
    <MarkdownBodySourceBlockContext.Provider value={sourceBlockTag}>
      <div
        data-source-block="true"
        data-source-start-line={range.startLine}
        data-source-end-line={range.endLine}
        className="relative min-w-0"
      >
        {gutterButton}
        {commentMarker}
        {hostElement}
        {indentedCommentThread}
      </div>
    </MarkdownBodySourceBlockContext.Provider>
  )
}
