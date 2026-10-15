import { useState } from 'react'

import { MarkdownBody } from '#components/note-detail/markdown-body'
import {
  type DiffCommentAnchor,
  getDiffCommentAnchorKey,
} from '#components/note-detail/note-version-line-comment-group'
import { useNoteVersionComments } from '#components/note-detail/use-note-version-comments'
import type { ResolvedRef } from '#hooks/use-resolve-ref'
import type { components } from '#lib/api/schema.gen'

type ApiGraphDef = components['schemas']['GraphDef']
type NoteLinkItem = components['schemas']['NoteLinkItem']

const EMPTY_GRAPHS: ApiGraphDef[] = []
const EMPTY_NOTE_LINKS: NoteLinkItem[] = []

interface NoteVersionBodyCommentsProps {
  versionId: string
  source: string
  graphs?: ApiGraphDef[]
  noteLinks?: NoteLinkItem[]
  resolvedPriceReferences?: components['schemas']['Value']
  onAnno?: (id: string) => void
  onRef?: (token: string, resolved: ResolvedRef) => void
}

export function NoteVersionBodyComments({
  versionId,
  source,
  graphs = EMPTY_GRAPHS,
  noteLinks = EMPTY_NOTE_LINKS,
  resolvedPriceReferences,
  onAnno,
  onRef,
}: NoteVersionBodyCommentsProps) {
  const [activeCommentAnchor, setActiveCommentAnchor] =
    useState<DiffCommentAnchor | null>(null)
  const [replyingCommentId, setReplyingCommentId] = useState<string | null>(
    null,
  )
  const commentState = useNoteVersionComments(versionId)

  return (
    <MarkdownBody
      source={source}
      graphs={graphs}
      noteLinks={noteLinks}
      resolvedPriceReferences={resolvedPriceReferences}
      onAnno={onAnno}
      onRef={onRef}
      rangeComments={{
        comments: commentState.comments,
        activeCommentAnchor,
        activeCommentKey:
          activeCommentAnchor == null
            ? null
            : getDiffCommentAnchorKey(activeCommentAnchor),
        replyingCommentId,
        isCreating: commentState.isCreating,
        isReplying: commentState.isReplying,
        resolvingCommentId: commentState.resolvingCommentId,
        hasCreateError: commentState.hasCreateError,
        hasReplyError: commentState.hasReplyError,
        hasResolveError: commentState.hasResolveError,
        isCommentsPending: commentState.isCommentsPending,
        hasCommentsError: commentState.hasCommentsError,
        onStartComment: (anchor) => {
          commentState.resetCreateError()
          commentState.resetReplyError()
          setReplyingCommentId(null)
          setActiveCommentAnchor(anchor)
        },
        onCancelComment: () => {
          setActiveCommentAnchor(null)
        },
        onCreateComment: (anchor, body) => {
          commentState.submitComment({ ...anchor, side: 'new' }, body, () => {
            setActiveCommentAnchor(null)
          })
        },
        onStartReply: (commentId) => {
          commentState.resetCreateError()
          commentState.resetReplyError()
          setActiveCommentAnchor(null)
          setReplyingCommentId(commentId)
        },
        onCancelReply: () => {
          setReplyingCommentId(null)
        },
        onReply: (parentId, body) => {
          commentState.submitReply(parentId, body, () => {
            setReplyingCommentId(null)
          })
        },
        onToggleResolved: commentState.toggleResolved,
      }}
    />
  )
}
