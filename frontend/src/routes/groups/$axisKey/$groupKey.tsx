import { useQuery } from '@tanstack/react-query'
import { createFileRoute } from '@tanstack/react-router'
import { useMemo } from 'react'

import { GroupDetailPage } from '#components/group-detail-page'
import { $api, fetchClient } from '#lib/api/client'

export const Route = createFileRoute('/groups/$axisKey/$groupKey')({
  component: GroupDetailRoute,
})

function GroupDetailRoute() {
  const { axisKey, groupKey } = Route.useParams()
  const groupQuery = useQuery({
    queryKey: ['group-detail', axisKey, groupKey],
    queryFn: async ({ signal }) => {
      const result = await fetchClient.GET(
        '/api/refs/groups/{axis_key}/{group_key}',
        {
          params: { path: { axis_key: axisKey, group_key: groupKey } },
          signal,
        },
      )
      if (result.response.status === 404) return { kind: 'not-found' as const }
      if (!result.response.ok || result.data == null) {
        return { kind: 'error' as const }
      }
      return { kind: 'loaded' as const, group: result.data }
    },
  })
  const notesQuery = $api.useQuery(
    'get',
    '/api/notes',
    { params: { query: { ref: `group:${axisKey}/${groupKey}` } } },
    { enabled: groupQuery.data?.kind === 'loaded' },
  )
  const summaryQuery = $api.useQuery('get', '/api/trades/summary')
  const heldStockIds = useMemo(
    () =>
      (summaryQuery.data?.positions ?? [])
        .filter((position) => position.qty > 0)
        .map((position) => position.symbol),
    [summaryQuery.data],
  )

  if (groupQuery.isPending) {
    return (
      <GroupDetailPage state="loading" axisKey={axisKey} groupKey={groupKey} />
    )
  }

  if (groupQuery.isError) {
    return (
      <GroupDetailPage state="error" axisKey={axisKey} groupKey={groupKey} />
    )
  }

  const groupResult = groupQuery.data
  if (groupResult.kind === 'error') {
    return (
      <GroupDetailPage state="error" axisKey={axisKey} groupKey={groupKey} />
    )
  }

  if (groupResult.kind === 'not-found') {
    return (
      <GroupDetailPage
        state="not-found"
        axisKey={axisKey}
        groupKey={groupKey}
      />
    )
  }

  return (
    <GroupDetailPage
      state="loaded"
      group={groupResult.group}
      heldStockIds={heldStockIds}
      notes={notesQuery.data ?? null}
      isNotesError={notesQuery.isError}
    />
  )
}
