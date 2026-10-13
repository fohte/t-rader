import { useMemo } from 'react'

import {
  buildReviewQueueData,
  type ReviewQueueData,
} from '#components/review/review-queue-model'
import { $api } from '#lib/api/client'

export function useReviewQueue(): {
  data: ReviewQueueData | null
  isPending: boolean
  isError: boolean
} {
  const versionsQuery = $api.useQuery('get', '/api/note-versions/pending')
  const annotationsQuery = $api.useQuery('get', '/api/annotations', {
    params: { query: { status: 'unread' } },
  })
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

  return { data, isPending, isError }
}
