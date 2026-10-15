import { createContext, type ReactNode } from 'react'

import type { DiffCommentAnchor } from '#components/note-detail/note-version-line-comment-group'
import type { SourceLineRange } from '#components/note-detail/use-markdown-body-comment-selection'
import type { NoteVersionComment } from '#lib/api/note-version-types'

export const NOTE_BODY_INDENT_REM = 1.25

export interface MarkdownBodyRangeCommentProps {
  comments: NoteVersionComment[]
  activeCommentAnchor: DiffCommentAnchor | null
  activeCommentKey: string | null
  replyingCommentId: string | null
  isCreating: boolean
  isReplying: boolean
  resolvingCommentId: string | null
  hasCreateError: boolean
  hasReplyError: boolean
  hasResolveError?: boolean
  isCommentsPending: boolean
  hasCommentsError: boolean
  onStartComment: (anchor: DiffCommentAnchor) => void
  onCancelComment: () => void
  onCreateComment: (anchor: DiffCommentAnchor, body: string) => void
  onStartReply: (commentId: string) => void
  onCancelReply: () => void
  onReply: (commentId: string, body: string) => void
  onToggleResolved: (comment: NoteVersionComment) => void
}

export interface MarkdownBodyRangeCommentContextValue {
  props: MarkdownBodyRangeCommentProps
  selectedRange: SourceLineRange | null
  onGutterPointerDown: (range: SourceLineRange) => void
  onGutterClick: (range: SourceLineRange, shiftKey: boolean) => void
}

export const MarkdownBodyRangeCommentContext =
  createContext<MarkdownBodyRangeCommentContextValue | null>(null)

export const MarkdownBodyTableCellGutterContext = createContext<ReactNode>(null)

export const MarkdownBodySourceBlockContext = createContext<string | null>(null)
