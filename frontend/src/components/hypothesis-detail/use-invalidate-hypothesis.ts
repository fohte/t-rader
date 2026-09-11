import { useQueryClient } from '@tanstack/react-query'

import { $api } from '#lib/api/client'

export function useInvalidateHypothesis(
  hypothesisId: string,
  strategyId: string | null,
) {
  const queryClient = useQueryClient()
  return () => {
    void queryClient.invalidateQueries({
      queryKey: $api.queryOptions('get', '/api/hypotheses/{hypothesis_id}', {
        params: { path: { hypothesis_id: hypothesisId } },
      }).queryKey,
    })
    void queryClient.invalidateQueries({
      queryKey: $api.queryOptions('get', '/api/hypotheses').queryKey,
    })
    if (strategyId != null) {
      void queryClient.invalidateQueries({
        queryKey: $api.queryOptions('get', '/api/strategies/{id}/hypotheses', {
          params: { path: { id: strategyId } },
        }).queryKey,
      })
    }
  }
}
