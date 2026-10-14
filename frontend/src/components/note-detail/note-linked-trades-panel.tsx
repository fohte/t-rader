import { useQueries } from '@tanstack/react-query'
import { useMemo } from 'react'

import { NoteLinkedTradesPanelView } from '#components/note-detail/note-linked-trades-panel-view'
import { $api } from '#lib/api/client'

export function NoteLinkedTradesPanel({ noteId }: { noteId: string }) {
  const tradesQuery = $api.useQuery('get', '/api/notes/{id}/trades', {
    params: { path: { id: noteId } },
  })
  const symbols = useMemo(
    () =>
      Array.from(
        new Set((tradesQuery.data ?? []).map((trade) => trade.symbol)),
      ),
    [tradesQuery.data],
  )
  const stockQueries = useQueries({
    queries: symbols.map((id) =>
      $api.queryOptions('get', '/api/refs/stocks/{id}', {
        params: { path: { id } },
      }),
    ),
  })
  const stockNameById = useMemo(
    () =>
      new Map(
        stockQueries.flatMap((query) =>
          query.data != null ? [[query.data.id, query.data.name]] : [],
        ),
      ),
    [stockQueries],
  )

  if (tradesQuery.isPending) {
    return <NoteLinkedTradesPanelView state="loading" />
  }
  if (tradesQuery.isError) {
    return <NoteLinkedTradesPanelView state="error" />
  }
  if (tradesQuery.data.length === 0) return null

  return (
    <NoteLinkedTradesPanelView
      state="ready"
      trades={tradesQuery.data}
      stockNameById={stockNameById}
    />
  )
}
