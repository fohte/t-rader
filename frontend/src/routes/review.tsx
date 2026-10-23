import { createFileRoute } from '@tanstack/react-router'
import { useEffect, useState } from 'react'

import { ReviewPageView } from '#components/review/review-page-view'
import {
  type ReviewNavigationSnapshot,
  type ReviewQueueItem,
  reviewQueueModel,
  type ReviewTypeFilter,
} from '#components/review/review-queue-model'
import {
  type ReviewApprovalCallbacks,
  useReviewApproval,
} from '#components/review/use-review-approval'
import { useReviewQueue } from '#components/review/use-review-queue'
import { $api } from '#lib/api/client'

export const Route = createFileRoute('/review')({
  component: ReviewPage,
})

function ReviewPage() {
  const [typeFilter, setTypeFilter] = useState<ReviewTypeFilter>('all')
  const [noteKindFilter, setNoteKindFilter] = useState('all')
  const { data, isPending, isError, invalidate } = useReviewQueue()
  const noteKindsQuery = $api.useQuery('get', '/api/note-kinds')
  const approveVersion = $api.useMutation(
    'post',
    '/api/notes/{id}/versions/{n}/approve',
  )
  const approveAnnotation = $api.useMutation(
    'post',
    '/api/annotations/{id}/approve',
  )
  function approveItem(
    item: ReviewQueueItem,
    callbacks: ReviewApprovalCallbacks,
  ): void {
    if (item.kind === 'note') {
      approveVersion.mutate(
        {
          params: { path: { id: item.noteId, n: item.versionNo } },
          body: {},
        },
        callbacks,
      )
      return
    }

    approveAnnotation.mutate(
      { params: { path: { id: item.id } }, body: {} },
      callbacks,
    )
  }
  const approval = useReviewApproval({ data, approveItem, invalidate })
  const effectiveNoteKindFilter =
    data == null
      ? noteKindFilter
      : reviewQueueModel.normalizeNoteKindFilter(data.items, noteKindFilter)
  useEffect(() => {
    if (effectiveNoteKindFilter !== noteKindFilter) {
      setNoteKindFilter(effectiveNoteKindFilter)
    }
  }, [effectiveNoteKindFilter, noteKindFilter])
  const [openedNavigationSnapshot, setOpenedNavigationSnapshot] =
    useState<ReviewNavigationSnapshot | null>(null)
  useEffect(() => {
    if (openedNavigationSnapshot == null && data != null) {
      setOpenedNavigationSnapshot(
        reviewQueueModel.createNavigationSnapshot(data.items),
      )
    }
  }, [data, openedNavigationSnapshot])
  const navigationSnapshot =
    openedNavigationSnapshot ??
    (data == null
      ? null
      : reviewQueueModel.createNavigationSnapshot(data.items))

  return (
    <ReviewPageView
      data={data}
      noteKinds={noteKindsQuery.data ?? []}
      noteKindCatalogState={
        noteKindsQuery.isPending
          ? 'loading'
          : noteKindsQuery.isError
            ? 'error'
            : 'loaded'
      }
      navigationSnapshot={navigationSnapshot}
      isPending={isPending}
      isError={isError}
      typeFilter={typeFilter}
      noteKindFilter={effectiveNoteKindFilter}
      selectedItemIds={approval.state.selectedItemIds}
      isApproving={approval.state.isApproving}
      hasApprovalError={approval.state.hasApprovalError}
      onTypeFilterChange={(filter) => {
        setTypeFilter(filter)
        approval.actions.clearSelection()
      }}
      onNoteKindFilterChange={(filter) => {
        setNoteKindFilter(filter)
        approval.actions.clearSelection()
      }}
      onToggleItem={approval.actions.toggleItem}
      onToggleVisibleItems={approval.actions.toggleVisibleItems}
      onApproveSelected={approval.actions.approveSelected}
    />
  )
}
