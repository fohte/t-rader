import { useQueryClient } from '@tanstack/react-query'

import { $api } from '@/lib/api/client'

/** Trade 関連クエリ (一覧 + サマリ) をまとめて invalidate するヘルパ。 */
export function useInvalidateTrades() {
  const queryClient = useQueryClient()
  return () => {
    void queryClient.invalidateQueries({
      queryKey: $api.queryOptions('get', '/api/trades').queryKey,
    })
    void queryClient.invalidateQueries({
      queryKey: $api.queryOptions('get', '/api/trades/summary').queryKey,
    })
  }
}
