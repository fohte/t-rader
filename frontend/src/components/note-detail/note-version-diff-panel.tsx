import { useState } from 'react'

import {
  type DiffColumnMode,
  NoteVersionDiffView,
} from '#components/note-detail/note-version-diff-view'
import { getDiffCommentAnchorKey } from '#components/note-detail/note-version-line-comment-group'
import { useNoteVersionComments } from '#components/note-detail/use-note-version-comments'
import type { NoteVersion } from '#lib/api/note-version-types'
import { buildNoteVersionDiff } from '#lib/note-version-diff'

interface NoteVersionDiffPanelProps {
  version: NoteVersion
  previousVersion: NoteVersion | null
}

export function NoteVersionDiffPanel({
  version,
  previousVersion,
}: NoteVersionDiffPanelProps) {
  const [mode, setMode] = useState<DiffColumnMode>('one-column')
  const [activeCommentKey, setActiveCommentKey] = useState<string | null>(null)
  const [replyingCommentId, setReplyingCommentId] = useState<string | null>(
    null,
  )
  const commentState = useNoteVersionComments(version.id)
  const diffRows = buildNoteVersionDiff(
    previousVersion?.body_md ?? null,
    version.body_md,
  )

  return (
    <NoteVersionDiffView
      rows={diffRows}
      comments={commentState.comments}
      mode={mode}
      activeCommentKey={activeCommentKey}
      replyingCommentId={replyingCommentId}
      isCreating={commentState.isCreating}
      isReplying={commentState.isReplying}
      resolvingCommentId={commentState.resolvingCommentId}
      hasCreateError={commentState.hasCreateError}
      hasReplyError={commentState.hasReplyError}
      isCommentsPending={commentState.isCommentsPending}
      hasCommentsError={commentState.hasCommentsError}
      onModeChange={setMode}
      onStartComment={(anchor) => {
        commentState.resetCreateError()
        commentState.resetReplyError()
        setReplyingCommentId(null)
        setActiveCommentKey(getDiffCommentAnchorKey(anchor))
      }}
      onCancelComment={() => {
        setActiveCommentKey(null)
      }}
      onCreateComment={(anchor, body) => {
        commentState.submitComment(anchor, body, () => {
          setActiveCommentKey(null)
        })
      }}
      onStartReply={(commentId) => {
        commentState.resetCreateError()
        commentState.resetReplyError()
        setActiveCommentKey(null)
        setReplyingCommentId(commentId)
      }}
      onCancelReply={() => {
        setReplyingCommentId(null)
      }}
      onReply={(parentId, body) => {
        commentState.submitReply(parentId, body, () => {
          setReplyingCommentId(null)
        })
      }}
      onToggleResolved={commentState.toggleResolved}
    />
  )
}
