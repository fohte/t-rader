import { useMemo } from 'react'

import {
  buildReviewQueueData,
  type ReviewQueueData,
} from '#components/dashboard/dashboard-model'
import { $api } from '#lib/api/client'

export function useReviewQueue(): {
  data: ReviewQueueData | null
  isPending: boolean
  isError: boolean
} {
  const notesQuery = $api.useQuery('get', '/api/notes', {
    params: { query: { status: 'unread' } },
  })
  const versionsQuery = $api.useQuery('get', '/api/note-versions/pending')
  const annotationsQuery = $api.useQuery('get', '/api/annotations')
  const isPending =
    notesQuery.isPending ||
    versionsQuery.isPending ||
    annotationsQuery.isPending
  const isError =
    notesQuery.isError || versionsQuery.isError || annotationsQuery.isError
  const data = useMemo(
    () =>
      notesQuery.data == null ||
      versionsQuery.data == null ||
      annotationsQuery.data == null
        ? null
        : buildReviewQueueData({
            notes: notesQuery.data,
            noteVersions: versionsQuery.data,
            annotations: annotationsQuery.data,
          }),
    [notesQuery.data, versionsQuery.data, annotationsQuery.data],
  )

  return { data, isPending, isError }
}
