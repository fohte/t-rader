import { QueryClient } from '@tanstack/react-query'

import { getApiErrorStatus } from '#lib/api/error'

export function createQueryClient(): QueryClient {
  return new QueryClient({
    defaultOptions: {
      queries: {
        staleTime: 1000 * 60,
        retry: (failureCount, error) => {
          const status = getApiErrorStatus(error)
          if (status !== null && status >= 400 && status < 500) return false
          return failureCount < 3
        },
      },
    },
  })
}
