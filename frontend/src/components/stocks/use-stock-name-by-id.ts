import { useQueries } from '@tanstack/react-query'
import { useMemo } from 'react'

import { $api } from '#lib/api/client'

export function useStockNameById(ids: string[]) {
  const uniqueIds = useMemo(() => Array.from(new Set(ids)), [ids])
  const stockQueries = useQueries({
    queries: uniqueIds.map((id) =>
      $api.queryOptions('get', '/api/refs/stocks/{id}', {
        params: { path: { id } },
      }),
    ),
  })

  return useMemo(
    () =>
      new Map(
        stockQueries.flatMap((query) =>
          query.data != null ? [[query.data.id, query.data.name]] : [],
        ),
      ),
    [stockQueries],
  )
}
