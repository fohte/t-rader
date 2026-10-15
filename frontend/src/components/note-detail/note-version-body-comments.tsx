import { useQueryClient } from '@tanstack/react-query'
import { useState } from 'react'

import { MarkdownBody } from '#components/note-detail/markdown-body'
import {
  type DiffCommentAnchor,
  getDiffCommentAnchorKey,
} from '#components/note-detail/note-version-line-comment-group'
import type { ResolvedRef } from '#hooks/use-resolve-ref'
import { $api } from '#lib/api/client'
import type { NoteVersionComment } from '#lib/api/note-version-types'
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
  const queryClient = useQueryClient()
  const [activeCommentAnchor, setActiveCommentAnchor] =
    useState<DiffCommentAnchor | null>(null)
  const [replyingCommentId, setReplyingCommentId] = useState<string | null>(
    null,
  )
  const {
    data: comments,
    isPending,
    isError,
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

  const submitComment = (anchor: DiffCommentAnchor, body: string): void => {
    createComment.mutate(
      {
        body: {
          target_kind: 'note_version',
          target_id: versionId,
          body,
          anchor_text: anchor.text,
          anchor_side: 'new',
          start_line: anchor.lineNumber,
          end_line: anchor.endLineNumber ?? anchor.lineNumber,
        },
      },
      {
        onSuccess: () => {
          setActiveCommentAnchor(null)
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
          target_id: versionId,
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

  return (
    <MarkdownBody
      source={source}
      graphs={graphs}
      noteLinks={noteLinks}
      resolvedPriceReferences={resolvedPriceReferences}
      onAnno={onAnno}
      onRef={onRef}
      rangeComments={{
        comments: comments ?? [],
        activeCommentAnchor,
        activeCommentKey:
          activeCommentAnchor == null
            ? null
            : getDiffCommentAnchorKey(activeCommentAnchor),
        replyingCommentId,
        isCreating: createComment.isPending,
        isReplying: replyComment.isPending,
        resolvingCommentId,
        hasCreateError: createComment.isError,
        hasReplyError: replyComment.isError,
        isCommentsPending: isPending,
        hasCommentsError: isError,
        onStartComment: (anchor) => {
          createComment.reset()
          replyComment.reset()
          setReplyingCommentId(null)
          setActiveCommentAnchor(anchor)
        },
        onCancelComment: () => {
          setActiveCommentAnchor(null)
        },
        onCreateComment: submitComment,
        onStartReply: (commentId) => {
          createComment.reset()
          replyComment.reset()
          setActiveCommentAnchor(null)
          setReplyingCommentId(commentId)
        },
        onCancelReply: () => {
          setReplyingCommentId(null)
        },
        onReply: submitReply,
        onToggleResolved: toggleResolved,
      }}
    />
  )
}
