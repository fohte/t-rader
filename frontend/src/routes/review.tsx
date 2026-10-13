import { useQueryClient } from '@tanstack/react-query'
import { createFileRoute } from '@tanstack/react-router'
import { useEffect, useMemo, useState } from 'react'

import {
  ReviewPageView,
  type ReviewTypeFilter,
} from '#components/review/review-page-view'
import {
  buildReviewQueueData,
  createReviewNavigationSnapshot,
  type ReviewNavigationSnapshot,
  type ReviewQueueItem,
  reviewQueueItemKey,
} from '#components/review/review-queue-model'
import { $api } from '#lib/api/client'

export const Route = createFileRoute('/review')({
  component: ReviewPage,
})

function ReviewPage() {
  const queryClient = useQueryClient()
  const [typeFilter, setTypeFilter] = useState<ReviewTypeFilter>('all')
  const [noteKindFilter, setNoteKindFilter] = useState('all')
  const [selectedItemIds, setSelectedItemIds] = useState<string[]>([])
  const [isApproving, setIsApproving] = useState(false)
  const [hasApprovalError, setHasApprovalError] = useState(false)

  const versionsQuery = $api.useQuery('get', '/api/note-versions/pending')
  const annotationsQuery = $api.useQuery('get', '/api/annotations', {
    params: { query: { status: 'unread' } },
  })
  const approveVersion = $api.useMutation(
    'post',
    '/api/notes/{id}/versions/{n}/approve',
  )
  const approveAnnotation = $api.useMutation(
    'post',
    '/api/annotations/{id}/approve',
  )
  const isPending = versionsQuery.isPending || annotationsQuery.isPending
  const isError = versionsQuery.isError || annotationsQuery.isError
  const data = useMemo(
    () =>
      versionsQuery.data == null || annotationsQuery.data == null
        ? null
        : buildReviewQueueData({
            noteVersions: versionsQuery.data,
            annotations: annotationsQuery.data,
          }),
    [versionsQuery.data, annotationsQuery.data],
  )
  const [openedNavigationSnapshot, setOpenedNavigationSnapshot] =
    useState<ReviewNavigationSnapshot | null>(null)
  useEffect(() => {
    if (openedNavigationSnapshot == null && data != null) {
      setOpenedNavigationSnapshot(createReviewNavigationSnapshot(data.items))
    }
  }, [data, openedNavigationSnapshot])
  const navigationSnapshot =
    openedNavigationSnapshot ??
    (data == null ? null : createReviewNavigationSnapshot(data.items))

  function invalidateReviewData(): void {
    void queryClient.invalidateQueries({
      queryKey: $api.queryOptions('get', '/api/note-versions/pending').queryKey,
    })
    void queryClient.invalidateQueries({
      queryKey: $api.queryOptions('get', '/api/annotations', {
        params: { query: { status: 'unread' } },
      }).queryKey,
    })
  }

  function approveInOrder(items: ReviewQueueItem[], index: number): void {
    const item = items[index]
    if (item == null) {
      setIsApproving(false)
      setSelectedItemIds([])
      invalidateReviewData()
      return
    }

    const onSuccess = () => {
      const itemKey = reviewQueueItemKey(item)
      setSelectedItemIds((selected) =>
        selected.filter((selectedKey) => selectedKey !== itemKey),
      )
      approveInOrder(items, index + 1)
    }
    const onError = () => {
      setIsApproving(false)
      setHasApprovalError(true)
      invalidateReviewData()
    }

    if (item.kind === 'note') {
      approveVersion.mutate(
        {
          params: {
            path: { id: item.noteId, n: item.versionNo },
          },
          body: {},
        },
        { onSuccess, onError },
      )
      return
    }

    approveAnnotation.mutate(
      {
        params: { path: { id: item.id } },
        body: {},
      },
      { onSuccess, onError },
    )
  }

  function approveSelected(): void {
    if (data == null || isApproving) return
    const selected = new Set(selectedItemIds)
    const selectedItems = data.items.filter((item) =>
      selected.has(reviewQueueItemKey(item)),
    )
    if (selectedItems.length === 0) return
    setHasApprovalError(false)
    setIsApproving(true)
    approveInOrder(selectedItems, 0)
  }

  return (
    <ReviewPageView
      data={data}
      navigationSnapshot={navigationSnapshot}
      isPending={isPending}
      isError={isError}
      typeFilter={typeFilter}
      noteKindFilter={noteKindFilter}
      selectedItemIds={selectedItemIds}
      isApproving={isApproving}
      hasApprovalError={hasApprovalError}
      onTypeFilterChange={(filter) => {
        setTypeFilter(filter)
        setSelectedItemIds([])
      }}
      onNoteKindFilterChange={(filter) => {
        setNoteKindFilter(filter)
        setSelectedItemIds([])
      }}
      onToggleItem={(itemKey, checked) => {
        setSelectedItemIds((selected) =>
          checked
            ? [...new Set([...selected, itemKey])]
            : selected.filter((selectedKey) => selectedKey !== itemKey),
        )
      }}
      onToggleVisibleItems={(itemKeys, checked) => {
        setSelectedItemIds((selected) =>
          checked
            ? [...new Set([...selected, ...itemKeys])]
            : selected.filter((selectedKey) => !itemKeys.includes(selectedKey)),
        )
      }}
      onApproveSelected={approveSelected}
    />
  )
}
