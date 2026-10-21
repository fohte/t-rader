import { Button } from '@fohte/ui/button'
import {
  Children,
  cloneElement,
  type CSSProperties,
  isValidElement,
  type ReactNode,
  useContext,
  useRef,
} from 'react'
import { type ExtraProps } from 'react-markdown'

import {
  MarkdownBodyRangeCommentContext,
  type MarkdownBodyRangeCommentProps,
  MarkdownBodySourceBlockContext,
  MarkdownBodyTableCellGutterContext,
  NOTE_BODY_INDENT_REM,
} from '#components/note-detail/markdown-body-range-comment-context'
import {
  type CommentBlockPresentation,
  commentBlockPresentation,
} from '#components/note-detail/markdown-body-range-comment-presentation'
import { highlightFirstTextMatch } from '#components/note-detail/markdown-body-text-highlight'
import { LineCommentGroup } from '#components/note-detail/note-version-line-comment-group'
import type { SourceLineRange } from '#components/note-detail/use-markdown-body-comment-selection'
import { cn } from '#lib/utils'

type MarkdownNode = NonNullable<ExtraProps['node']>

const DEFAULT_GUTTER_MARKER_CLASS_NAME =
  'opacity-0 group-[:hover:not(:has([data-source-block]:hover))]:opacity-100'
const LIST_GUTTER_MARKER_CLASS_NAME = 'opacity-0'
const LIST_RANGE_BLOCK_CLASS_NAME = 'note-body-list-range-block'

type RangeBlockStyle = CSSProperties & {
  '--markdown-body-list-offset'?: string
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

interface RangeBlockGutterButtonProps {
  range: SourceLineRange
  isMarked: boolean
  onPointerDown: (range: SourceLineRange) => void
  onClick: (range: SourceLineRange, shiftKey: boolean) => void
  sourceBlockTag: string
  isTableRow?: boolean
  listOffset?: string
}

function RangeBlockGutterButton({
  range,
  isMarked,
  onPointerDown,
  onClick,
  sourceBlockTag,
  isTableRow = false,
  listOffset,
}: RangeBlockGutterButtonProps) {
  const pointerTypeRef = useRef<string | null>(null)
  const positionClassName = isTableRow
    ? '-left-11 top-1.5'
    : cn(
        '-left-11',
        sourceBlockTag === 'li' && 'translate-x-(--markdown-body-list-offset)',
        sourceBlockTag === 'li' && 'top-0',
        sourceBlockTag === 'pre' && 'top-3',
        sourceBlockTag === 'blockquote' && 'top-1.5',
      )
  const markerClassName =
    sourceBlockTag === 'li'
      ? LIST_GUTTER_MARKER_CLASS_NAME
      : DEFAULT_GUTTER_MARKER_CLASS_NAME
  const positionStyle: RangeBlockStyle | undefined =
    listOffset == null
      ? undefined
      : { '--markdown-body-list-offset': listOffset }

  return (
    <div
      className={cn('absolute z-10', positionClassName)}
      style={positionStyle}
    >
      <Button
        type="button"
        variant="plain"
        aria-label={`行 ${String(range.startLine)}${range.endLine > range.startLine ? ` から ${String(range.endLine)}` : ''} にコメント`}
        data-comment-text-ignore="true"
        data-source-gutter-button="true"
        onPointerDown={(event) => {
          pointerTypeRef.current = event.pointerType
          if (event.pointerType === 'touch') return
          onPointerDown(range)
        }}
        onPointerCancel={() => {
          pointerTypeRef.current = null
        }}
        onClick={(event) => {
          const pointerType =
            'pointerType' in event.nativeEvent
              ? event.nativeEvent.pointerType
              : null
          const isTouchClick =
            pointerType === 'touch' ||
            (event.detail > 0 && pointerTypeRef.current === 'touch')
          pointerTypeRef.current = null
          if (isTouchClick) return
          onClick(range, event.shiftKey)
        }}
        className="min-h-7 w-11"
      >
        <span className="w-full pr-2 text-right">
          {isMarked ? (
            <span aria-hidden="true" className="select-none font-mono text-2xs">
              ●
            </span>
          ) : (
            <span
              aria-hidden="true"
              data-source-gutter-marker="true"
              className={cn('select-none font-mono text-2xs', markerClassName)}
            >
              +
            </span>
          )}
        </span>
      </Button>
    </div>
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
  const isListSourceBlock = sourceBlockTag === 'li'
  const isNestedSourceBlock =
    parentSourceBlock != null &&
    !(parentSourceBlock === 'li' && isListSourceBlock)
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

  const childRanges = descendantSourceBlockRanges(content, isListSourceBlock)
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
  const isBlockSelected =
    (presentation.isSelected && !presentation.isTextSelection) ||
    textHighlight?.match === 'missing'
  const renderedContent = textHighlight?.children ?? content
  const { props } = context
  const showThread = presentation.isActive || presentation.comments.length > 0
  const isMarked = presentation.isSelected || presentation.isCommented
  const listOffset = isListSourceBlock
    ? `-${String(listDepth * NOTE_BODY_INDENT_REM)}rem`
    : undefined
  const listIndentStyle: RangeBlockStyle | undefined =
    listOffset == null
      ? undefined
      : { '--markdown-body-list-offset': listOffset }
  const commentMarker = isMarked ? (
    <span
      aria-hidden="true"
      data-comment-text-ignore="true"
      className={cn(
        'pointer-events-none absolute inset-y-0 -left-9 w-0.5 bg-muted-foreground',
        isListSourceBlock && 'translate-x-(--markdown-body-list-offset)',
      )}
      style={listIndentStyle}
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
          isMarked={isMarked}
          onPointerDown={context.onGutterPointerDown}
          onClick={context.onGutterClick}
          sourceBlockTag={sourceBlockTag}
          isTableRow
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
            className={cn(
              rowClassName,
              'group',
              isBlockSelected && 'bg-muted-foreground/10',
            )}
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
        <div
          className={cn('min-w-0', isBlockSelected && 'bg-muted-foreground/10')}
        >
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

  const gutterButton = (
    <RangeBlockGutterButton
      range={range}
      isMarked={isMarked}
      onPointerDown={context.onGutterPointerDown}
      onClick={context.onGutterClick}
      sourceBlockTag={sourceBlockTag}
      listOffset={listOffset}
    />
  )

  const isIntrinsicElement =
    isValidElement<RangeBlockElementProps>(renderedContent) &&
    typeof renderedContent.type === 'string'
  if (isIntrinsicElement && isListSourceBlock) {
    const hostElement = cloneElement(renderedContent, {
      className: cn(
        renderedContent.props.className,
        'group relative',
        LIST_RANGE_BLOCK_CLASS_NAME,
        isBlockSelected && 'bg-muted-foreground/10',
      ),
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
        className: cn(
          renderedContent.props.className,
          isBlockSelected && 'bg-muted-foreground/10',
        ),
      })
    : renderedContent

  return (
    <MarkdownBodySourceBlockContext.Provider value={sourceBlockTag}>
      <div
        data-source-block="true"
        data-source-start-line={range.startLine}
        data-source-end-line={range.endLine}
        className={cn(
          'group relative min-w-0',
          isListSourceBlock && LIST_RANGE_BLOCK_CLASS_NAME,
        )}
      >
        {gutterButton}
        {commentMarker}
        {hostElement}
        {indentedCommentThread}
      </div>
    </MarkdownBodySourceBlockContext.Provider>
  )
}
