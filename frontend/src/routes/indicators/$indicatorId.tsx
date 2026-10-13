import { useQuery } from '@tanstack/react-query'
import { createFileRoute } from '@tanstack/react-router'
import { useState } from 'react'

import { IndicatorDetailPage } from '#components/indicator-detail-page'
import { $api, fetchClient } from '#lib/api/client'
import type { IndicatorPeriod } from '#lib/indicator-observation'

export const Route = createFileRoute('/indicators/$indicatorId')({
  component: IndicatorDetailRoute,
})

function IndicatorDetailRoute() {
  const { indicatorId } = Route.useParams()
  const [period, setPeriod] = useState<IndicatorPeriod>('1y')
  // query client の error には HTTP status が含まれないため、404 をここで分岐する。
  const indicatorQuery = useQuery({
    queryKey: ['indicator-detail', indicatorId],
    queryFn: async ({ signal }) => {
      const result = await fetchClient.GET('/api/refs/indicators/{id}', {
        params: { path: { id: indicatorId } },
        signal,
      })
      if (result.response.status === 404) return { kind: 'not-found' as const }
      if (!result.response.ok || result.data == null) {
        return { kind: 'error' as const }
      }
      return { kind: 'loaded' as const, indicator: result.data }
    },
  })
  const observationsQuery = $api.useQuery(
    'get',
    '/api/refs/indicators/{id}/observations',
    { params: { path: { id: indicatorId }, query: {} } },
    { enabled: indicatorQuery.data?.kind === 'loaded' },
  )
  const notesQuery = $api.useQuery(
    'get',
    '/api/notes',
    { params: { query: { ref: `indicator:${indicatorId}` } } },
    { enabled: indicatorQuery.data?.kind === 'loaded' },
  )

  if (indicatorQuery.isPending) {
    return <IndicatorDetailPage state="loading" indicatorId={indicatorId} />
  }

  if (indicatorQuery.isError) {
    return <IndicatorDetailPage state="error" indicatorId={indicatorId} />
  }

  const indicatorResult = indicatorQuery.data
  if (indicatorResult.kind === 'error') {
    return <IndicatorDetailPage state="error" indicatorId={indicatorId} />
  }

  if (indicatorResult.kind === 'not-found') {
    return <IndicatorDetailPage state="not-found" indicatorId={indicatorId} />
  }

  return (
    <IndicatorDetailPage
      state="loaded"
      indicator={indicatorResult.indicator}
      observations={observationsQuery.data ?? null}
      isObservationsLoading={observationsQuery.isPending}
      isObservationsError={observationsQuery.isError}
      notes={notesQuery.data ?? null}
      isNotesError={notesQuery.isError}
      period={period}
      onPeriodChange={setPeriod}
    />
  )
}
