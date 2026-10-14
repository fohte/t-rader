import { useMemo } from 'react'

import { NoteLinkedTradesPanelView } from '#components/note-detail/note-linked-trades-panel-view'
import { useStockNameById } from '#components/stocks/use-stock-name-by-id'
import { $api } from '#lib/api/client'

export function NoteLinkedTradesPanel({ noteId }: { noteId: string }) {
  const tradesQuery = $api.useQuery('get', '/api/notes/{id}/trades', {
    params: { path: { id: noteId } },
  })
  const symbols = useMemo(
    () => (tradesQuery.data ?? []).map((trade) => trade.symbol),
    [tradesQuery.data],
  )
  const stockNameById = useStockNameById(symbols)

  if (tradesQuery.isPending) {
    return <NoteLinkedTradesPanelView state="loading" />
  }
  if (tradesQuery.isError) {
    return <NoteLinkedTradesPanelView state="error" />
  }
  return (
    <NoteLinkedTradesPanelView
      state="ready"
      trades={tradesQuery.data}
      stockNameById={stockNameById}
    />
  )
}
