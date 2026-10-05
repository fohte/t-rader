import { useQueries } from '@tanstack/react-query'
import { useMemo } from 'react'

import { PredictionsPanelView } from '#components/note-detail/predictions-panel-view'
import { $api } from '#lib/api/client'

interface PredictionsPanelProps {
  noteId: string
}

export function PredictionsPanel({ noteId }: PredictionsPanelProps) {
  const { data: predictions } = $api.useQuery(
    'get',
    '/api/notes/{id}/predictions',
    { params: { path: { id: noteId } } },
  )

  const stockIds = useMemo(
    () =>
      Array.from(
        new Set(
          (predictions ?? []).flatMap((p) => [
            p.target_stock_id,
            p.benchmark_stock_id,
          ]),
        ),
      ),
    [predictions],
  )
  const stockQueries = useQueries({
    queries: stockIds.map((id) =>
      $api.queryOptions('get', '/api/refs/stocks/{id}', {
        params: { path: { id } },
      }),
    ),
  })
  const stockNameById = useMemo(
    () =>
      new Map(
        stockQueries.flatMap((q) =>
          q.data != null ? [[q.data.id, q.data.name]] : [],
        ),
      ),
    [stockQueries],
  )

  if (predictions == null || predictions.length === 0) return null

  return (
    <PredictionsPanelView
      predictions={predictions}
      stockNameById={stockNameById}
    />
  )
}
