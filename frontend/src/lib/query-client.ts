import { QueryClient } from '@tanstack/react-query'

function getHttpStatus(error: unknown): number | null {
  if (typeof error !== 'object' || error === null || !('status' in error)) {
    return null
  }

  return typeof error.status === 'number' ? error.status : null
}

export function createQueryClient(): QueryClient {
  return new QueryClient({
    defaultOptions: {
      queries: {
        // 不必要な再フェッチを抑制する
        staleTime: 1000 * 60,
        retry: (failureCount, error) => {
          const status = getHttpStatus(error)
          if (status !== null && status >= 400 && status < 500) return false
          return failureCount < 3
        },
      },
    },
  })
}
