import { useQueryClient } from '@tanstack/react-query'
import { useState } from 'react'

import {
  type DiffColumnMode,
  type DiffCommentAnchor,
  NoteVersionDiffView,
} from '#components/note-detail/note-version-diff-view'
import { $api } from '#lib/api/client'
import type {
  NoteVersion,
  NoteVersionComment,
} from '#lib/api/note-version-types'
import { buildNoteVersionDiff } from '#lib/note-version-diff'

interface NoteVersionDiffPanelProps {
  version: NoteVersion
  previousVersion: NoteVersion | null
}

export function NoteVersionDiffPanel({
  version,
  previousVersion,
}: NoteVersionDiffPanelProps) {
  const queryClient = useQueryClient()
  const [mode, setMode] = useState<DiffColumnMode>('one-column')
  const [activeCommentKey, setActiveCommentKey] = useState<string | null>(null)
  const [replyingCommentId, setReplyingCommentId] = useState<string | null>(
    null,
  )
  const {
    data: comments,
    isPending,
    isError,
  } = $api.useQuery('get', '/api/comments', {
    params: { query: { target_kind: 'note_version', target_id: version.id } },
  })
  const createComment = $api.useMutation('post', '/api/comments')
  const replyComment = $api.useMutation('post', '/api/comments')
  const resolveComment = $api.useMutation('patch', '/api/comments/{id}')

  const invalidateComments = (): void => {
    void queryClient.invalidateQueries({
      queryKey: $api.queryOptions('get', '/api/comments', {
        params: {
          query: { target_kind: 'note_version', target_id: version.id },
        },
      }).queryKey,
    })
  }

  const submitComment = (anchor: DiffCommentAnchor, body: string): void => {
    createComment.mutate(
      {
        body: {
          target_kind: 'note_version',
          target_id: version.id,
          body,
          anchor_text: anchor.text,
          anchor_side: anchor.side,
          start_line: anchor.lineNumber,
          end_line: anchor.lineNumber,
        },
      },
      {
        onSuccess: () => {
          setActiveCommentKey(null)
          invalidateComments()
        },
      },
    )
  }

  const submitReply = (parentId: string, body: string): void => {
    replyComment.mutate(
      {
        body: {
          target_kind: 'note_version',
          target_id: version.id,
          parent_id: parentId,
          body,
        },
      },
      {
        onSuccess: () => {
          setReplyingCommentId(null)
          invalidateComments()
        },
      },
    )
  }

  const toggleResolved = (comment: NoteVersionComment): void => {
    resolveComment.mutate(
      {
        params: { path: { id: comment.id } },
        body: { resolved: !comment.resolved },
      },
      { onSuccess: invalidateComments },
    )
  }

  const resolvingCommentId = resolveComment.isPending
    ? resolveComment.variables.params.path.id
    : null
  const diffRows = buildNoteVersionDiff(
    previousVersion?.body_md ?? null,
    version.body_md,
  )

  return (
    <NoteVersionDiffView
      rows={diffRows}
      comments={comments ?? []}
      mode={mode}
      activeCommentKey={activeCommentKey}
      replyingCommentId={replyingCommentId}
      isCreating={createComment.isPending}
      isReplying={replyComment.isPending}
      resolvingCommentId={resolvingCommentId}
      hasCreateError={createComment.isError}
      hasReplyError={replyComment.isError}
      isCommentsPending={isPending}
      hasCommentsError={isError}
      onModeChange={setMode}
      onStartComment={(anchor) => {
        createComment.reset()
        replyComment.reset()
        setReplyingCommentId(null)
        setActiveCommentKey(`${anchor.side}:${String(anchor.lineNumber)}`)
      }}
      onCancelComment={() => {
        setActiveCommentKey(null)
      }}
      onCreateComment={submitComment}
      onStartReply={(commentId) => {
        createComment.reset()
        replyComment.reset()
        setActiveCommentKey(null)
        setReplyingCommentId(commentId)
      }}
      onCancelReply={() => {
        setReplyingCommentId(null)
      }}
      onReply={submitReply}
      onToggleResolved={toggleResolved}
    />
  )
}
