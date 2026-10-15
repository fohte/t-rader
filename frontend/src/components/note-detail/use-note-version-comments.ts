import { useQueryClient } from '@tanstack/react-query'

import type { DiffCommentAnchor } from '#components/note-detail/note-version-line-comment-group'
import { $api } from '#lib/api/client'
import type { NoteVersionComment } from '#lib/api/note-version-types'

export function useNoteVersionComments(versionId: string) {
  const queryClient = useQueryClient()
  const {
    data: comments,
    isPending: isCommentsPending,
    isError: hasCommentsError,
  } = $api.useQuery('get', '/api/comments', {
    params: { query: { target_kind: 'note_version', target_id: versionId } },
  })
  const createComment = $api.useMutation('post', '/api/comments')
  const replyComment = $api.useMutation('post', '/api/comments')
  const resolveComment = $api.useMutation('patch', '/api/comments/{id}')

  const invalidateComments = (): void => {
    void queryClient.invalidateQueries({
      queryKey: $api.queryOptions('get', '/api/comments', {
        params: {
          query: { target_kind: 'note_version', target_id: versionId },
        },
      }).queryKey,
    })
  }

  const submitComment = (
    anchor: DiffCommentAnchor,
    body: string,
    onSuccess?: () => void,
  ): void => {
    createComment.mutate(
      {
        body: {
          target_kind: 'note_version',
          target_id: versionId,
          body,
          anchor_text: anchor.text,
          anchor_side: anchor.side,
          start_line: anchor.lineNumber,
          end_line: anchor.endLineNumber ?? anchor.lineNumber,
        },
      },
      {
        onSuccess: () => {
          invalidateComments()
          onSuccess?.()
        },
      },
    )
  }

  const submitReply = (
    parentId: string,
    body: string,
    onSuccess?: () => void,
  ): void => {
    replyComment.mutate(
      {
        body: {
          target_kind: 'note_version',
          target_id: versionId,
          parent_id: parentId,
          body,
        },
      },
      {
        onSuccess: () => {
          invalidateComments()
          onSuccess?.()
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

  return {
    comments: comments ?? [],
    isCommentsPending,
    hasCommentsError,
    isCreating: createComment.isPending,
    isReplying: replyComment.isPending,
    resolvingCommentId: resolveComment.isPending
      ? resolveComment.variables.params.path.id
      : null,
    hasCreateError: createComment.isError,
    hasReplyError: replyComment.isError,
    hasResolveError: resolveComment.isError,
    submitComment,
    submitReply,
    toggleResolved,
    resetCreateError: () => {
      createComment.reset()
    },
    resetReplyError: () => {
      replyComment.reset()
    },
  }
}
