import { useState } from 'react'

import { StockSearchView } from '#components/stocks/stock-search-view'
import { $api } from '#lib/api/client'

export function StockSearch() {
  const [open, setOpen] = useState(false)
  const [query, setQuery] = useState('')
  const normalizedQuery = query.trim()
  const stocksQuery = $api.useQuery(
    'get',
    '/api/refs/stocks',
    { params: { query: { q: normalizedQuery } } },
    { enabled: open && normalizedQuery !== '' },
  )

  return (
    <StockSearchView
      open={open}
      onOpenChange={setOpen}
      query={query}
      onQueryChange={setQuery}
      stocks={stocksQuery.data ?? []}
      isPending={stocksQuery.isPending}
      isError={stocksQuery.isError}
    />
  )
}
