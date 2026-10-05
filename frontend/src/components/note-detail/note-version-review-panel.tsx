import { useQueryClient } from '@tanstack/react-query'
import { useState } from 'react'

import { NoteVersionReviewView } from '#components/note-detail/note-version-review-view'
import { $api } from '#lib/api/client'
import type { NoteVersion } from '#lib/api/note-version-types'

interface NoteVersionReviewPanelProps {
  version: NoteVersion
}

export function NoteVersionReviewPanel({
  version,
}: NoteVersionReviewPanelProps) {
  const queryClient = useQueryClient()
  const [reason, setReason] = useState('')
  const { data: comments, isPending: isCommentsPending } = $api.useQuery(
    'get',
    '/api/comments',
    {
      params: {
        query: { target_kind: 'note_version', target_id: version.id },
      },
    },
  )
  const approve = $api.useMutation(
    'post',
    '/api/notes/{id}/versions/{n}/approve',
  )
  const reject = $api.useMutation('post', '/api/notes/{id}/versions/{n}/reject')
  const makeCurrent = $api.useMutation(
    'post',
    '/api/notes/{id}/versions/{n}/make-current',
  )
  const lineCommentCount = (comments ?? []).filter(
    (comment) => comment.parent_id == null && comment.start_line != null,
  ).length

  const invalidateReviewData = (): void => {
    void queryClient.invalidateQueries({
      queryKey: $api.queryOptions('get', '/api/notes/{id}/versions', {
        params: { path: { id: version.note_id } },
      }).queryKey,
    })
    void queryClient.invalidateQueries({
      queryKey: $api.queryOptions('get', '/api/notes/{id}/versions/{n}', {
        params: { path: { id: version.note_id, n: version.version_no } },
      }).queryKey,
    })
    void queryClient.invalidateQueries({
      queryKey: $api.queryOptions('get', '/api/note-versions/pending').queryKey,
    })
    void queryClient.invalidateQueries({
      queryKey: $api.queryOptions('get', '/api/notes/{id}', {
        params: { path: { id: version.note_id } },
      }).queryKey,
    })
    void queryClient.invalidateQueries({
      queryKey: $api.queryOptions('get', '/api/notes').queryKey,
    })
    void queryClient.invalidateQueries({
      queryKey: $api.queryOptions('get', '/api/history', {
        params: {
          query: {
            target_kind: 'note',
            target_id: version.note_id,
            limit: 50,
          },
        },
      }).queryKey,
    })
  }

  const onApprove = (): void => {
    approve.mutate(
      {
        params: {
          path: { id: version.note_id, n: version.version_no },
        },
        body: {},
      },
      { onSuccess: invalidateReviewData },
    )
  }

  const onReject = (): void => {
    const label = reason.trim()
    reject.mutate(
      {
        params: {
          path: { id: version.note_id, n: version.version_no },
        },
        body: label === '' ? {} : { label },
      },
      {
        onSuccess: () => {
          setReason('')
          invalidateReviewData()
        },
      },
    )
  }

  const onMakeCurrent = (): void => {
    makeCurrent.mutate(
      {
        params: {
          path: { id: version.note_id, n: version.version_no },
        },
      },
      { onSuccess: invalidateReviewData },
    )
  }

  return (
    <NoteVersionReviewView
      version={version}
      lineCommentCount={lineCommentCount}
      isCommentsPending={isCommentsPending}
      reason={reason}
      isApproving={approve.isPending}
      isRejecting={reject.isPending}
      isMakingCurrent={makeCurrent.isPending}
      hasError={approve.isError || reject.isError || makeCurrent.isError}
      onReasonChange={setReason}
      onApprove={onApprove}
      onReject={onReject}
      onMakeCurrent={onMakeCurrent}
    />
  )
}
