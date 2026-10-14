import { useQueryClient } from '@tanstack/react-query'
import { createFileRoute, useRouterState } from '@tanstack/react-router'
import { type SyntheticEvent, useEffect, useState } from 'react'

import { AnnotationDetail } from '#components/annotations/annotation-detail'
import {
  type ReviewNavigationSnapshot,
  reviewQueueItemKey,
  reviewQueueModel,
} from '#components/review/review-queue-model'
import { useReviewQueue } from '#components/review/use-review-queue'
import { useResolveRef } from '#hooks/use-resolve-ref'
import { $api } from '#lib/api/client'

export const Route = createFileRoute('/annotations/$annoId')({
  component: AnnotationDetailPage,
})

function AnnotationDetailPage() {
  const { annoId } = Route.useParams()
  return <AnnotationDetailPageContent key={annoId} annoId={annoId} />
}

function AnnotationDetailPageContent({ annoId }: { annoId: string }) {
  const queryClient = useQueryClient()
  const reviewQueue = useReviewQueue()
  const reviewQueueHistoryState = useRouterState({
    select: (routerState) =>
      reviewQueueModel.readNavigationState(routerState.location.state),
  })
  const currentItemKey = reviewQueueItemKey({ kind: 'annotation', id: annoId })
  const hasMatchingHistoryState =
    reviewQueueHistoryState?.currentItemKey === currentItemKey
  const [fallbackNavigation, setFallbackNavigation] = useState<{
    annoId: string
    snapshot: ReviewNavigationSnapshot
  } | null>(null)
  const fallbackSnapshot =
    fallbackNavigation?.annoId === annoId ? fallbackNavigation.snapshot : null

  useEffect(() => {
    if (
      hasMatchingHistoryState ||
      fallbackSnapshot != null ||
      reviewQueue.data == null
    ) {
      return
    }
    setFallbackNavigation({
      annoId,
      snapshot: reviewQueueModel.createNavigationSnapshot(
        reviewQueue.data.items,
      ),
    })
  }, [annoId, fallbackSnapshot, hasMatchingHistoryState, reviewQueue.data])

  const reviewNavigationSnapshot = hasMatchingHistoryState
    ? reviewQueueHistoryState.snapshot
    : fallbackSnapshot
  const reviewNavigation =
    reviewNavigationSnapshot == null || reviewQueue.data == null
      ? null
      : {
          items: reviewQueue.data.items,
          snapshot: reviewNavigationSnapshot,
          currentItemKey,
        }

  const { data: annotation, isPending: annoPending } = $api.useQuery(
    'get',
    '/api/annotations/{id}',
    { params: { path: { id: annoId } } },
  )

  const { data: comments } = $api.useQuery('get', '/api/comments', {
    params: { query: { target_kind: 'annotation', target_id: annoId } },
  })

  const { data: history } = $api.useQuery('get', '/api/history', {
    params: { query: { target_kind: 'annotation', target_id: annoId } },
  })

  const stockRef = useResolveRef(`stock:${annotation?.target_symbol ?? ''}`)
  const [rejectionReason, setRejectionReason] = useState('')

  const invalidate = () => {
    void queryClient.invalidateQueries({
      queryKey: $api.queryOptions('get', '/api/annotations/{id}', {
        params: { path: { id: annoId } },
      }).queryKey,
    })
    void queryClient.invalidateQueries({
      queryKey: $api.queryOptions('get', '/api/annotations').queryKey,
    })
    void queryClient.invalidateQueries({
      queryKey: $api.queryOptions('get', '/api/history').queryKey,
    })
    reviewQueue.invalidate()
  }

  const approveMutation = $api.useMutation(
    'post',
    '/api/annotations/{id}/approve',
    { onSuccess: invalidate },
  )
  const rejectMutation = $api.useMutation(
    'post',
    '/api/annotations/{id}/reject',
    {
      onSuccess: () => {
        setRejectionReason('')
        invalidate()
      },
    },
  )

  const createCommentMutation = $api.useMutation('post', '/api/comments', {
    onSuccess: () => {
      void queryClient.invalidateQueries({
        queryKey: $api.queryOptions('get', '/api/comments', {
          params: {
            query: { target_kind: 'annotation', target_id: annoId },
          },
        }).queryKey,
      })
    },
  })

  const [commentDraft, setCommentDraft] = useState('')

  function handleApprove() {
    if (approveMutation.isPending) return
    approveMutation.mutate({ params: { path: { id: annoId } }, body: {} })
  }

  function handleReject() {
    if (rejectMutation.isPending) return
    const label = rejectionReason.trim()
    rejectMutation.mutate({
      params: { path: { id: annoId } },
      body: label === '' ? {} : { label },
    })
  }

  function handleAddComment(event: SyntheticEvent<HTMLFormElement>) {
    event.preventDefault()
    const body = commentDraft.trim()
    if (body === '' || createCommentMutation.isPending) return
    createCommentMutation.mutate(
      {
        body: {
          target_kind: 'annotation',
          target_id: annoId,
          body,
        },
      },
      {
        onSuccess: () => {
          setCommentDraft('')
        },
      },
    )
  }

  return (
    <AnnotationDetail
      annotation={annotation}
      isPending={annoPending}
      reviewNavigation={reviewNavigation}
      reviewNavigationError={reviewQueue.isError}
      comments={comments}
      history={history}
      stockName={stockRef.name}
      commentDraft={commentDraft}
      isApprovalPending={approveMutation.isPending}
      isRejectionPending={rejectMutation.isPending}
      isCommentPending={createCommentMutation.isPending}
      approvalError={approveMutation.error}
      rejectionError={rejectMutation.error}
      commentError={createCommentMutation.error}
      rejectionReason={rejectionReason}
      onApprove={handleApprove}
      onReject={handleReject}
      onRejectionReasonChange={setRejectionReason}
      onAddComment={handleAddComment}
      onCommentDraftChange={(value) => {
        setCommentDraft(value)
      }}
    />
  )
}
