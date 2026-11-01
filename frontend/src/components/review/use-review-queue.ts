import { useQueryClient } from '@tanstack/react-query'
import { useMemo } from 'react'

import {
  type ReviewQueueData,
  reviewQueueModel,
} from '#components/review/review-queue-model'
import { getReviewQueueQueryKeys } from '#components/review/review-queue-query-keys'
import { $api } from '#lib/api/client'

export function useReviewQueue({
  enabled = true,
}: { enabled?: boolean } = {}): {
  data: ReviewQueueData | null
  isPending: boolean
  isError: boolean
  invalidate: () => void
} {
  const queryClient = useQueryClient()
  const versionsQuery = $api.useQuery(
    'get',
    '/api/note-versions/pending',
    undefined,
    { enabled },
  )
  const annotationsQuery = $api.useQuery(
    'get',
    '/api/annotations',
    {
      params: { query: { status: 'unread' } },
    },
    { enabled },
  )
  const isPending = versionsQuery.isPending || annotationsQuery.isPending
  const isError = versionsQuery.isError || annotationsQuery.isError
  const data = useMemo(
    () =>
      versionsQuery.data == null || annotationsQuery.data == null
        ? null
        : reviewQueueModel.buildData({
            noteVersions: versionsQuery.data,
            annotations: annotationsQuery.data,
          }),
    [versionsQuery.data, annotationsQuery.data],
  )

  function invalidate(): void {
    for (const queryKey of getReviewQueueQueryKeys()) {
      void queryClient.invalidateQueries({ queryKey })
    }
  }

  return { data, isPending, isError, invalidate }
}
