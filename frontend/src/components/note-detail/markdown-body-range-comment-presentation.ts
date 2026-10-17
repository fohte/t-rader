import type { MarkdownBodyRangeCommentContextValue } from '#components/note-detail/markdown-body-range-comment-context'
import {
  commentsForThreads,
  type DiffCommentAnchor,
  getDiffCommentAnchorKey,
} from '#components/note-detail/note-version-line-comment-group'
import type { SourceLineRange } from '#components/note-detail/use-markdown-body-comment-selection'
import type { NoteVersionComment } from '#lib/api/note-version-types'

export interface CommentBlockPresentation {
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

export function commentBlockPresentation(
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
