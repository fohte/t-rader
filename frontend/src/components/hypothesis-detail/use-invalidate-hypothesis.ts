import { useQueryClient } from '@tanstack/react-query'

import { $api } from '@/lib/api/client'

export function useInvalidateHypothesis(
  strategyId: string,
  hypothesisId: string,
) {
  const queryClient = useQueryClient()
  return () => {
    void queryClient.invalidateQueries({
      queryKey: $api.queryOptions(
        'get',
        '/api/strategies/{id}/hypotheses/{hypothesis_id}',
        { params: { path: { id: strategyId, hypothesis_id: hypothesisId } } },
      ).queryKey,
    })
    void queryClient.invalidateQueries({
      queryKey: $api.queryOptions('get', '/api/strategies/{id}/hypotheses', {
        params: { path: { id: strategyId } },
      }).queryKey,
    })
  }
}
