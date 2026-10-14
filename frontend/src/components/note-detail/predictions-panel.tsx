import { useMemo } from 'react'

import { PredictionsPanelView } from '#components/note-detail/predictions-panel-view'
import { useStockNameById } from '#components/stocks/use-stock-name-by-id'
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
      (predictions ?? []).flatMap((prediction) => [
        prediction.target_stock_id,
        prediction.benchmark_stock_id,
      ]),
    [predictions],
  )
  const stockNameById = useStockNameById(stockIds)

  if (predictions == null || predictions.length === 0) return null

  return (
    <PredictionsPanelView
      predictions={predictions}
      stockNameById={stockNameById}
    />
  )
}
