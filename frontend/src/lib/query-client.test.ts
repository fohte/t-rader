import { describe, expect, it } from 'vitest'

import { $api } from '#lib/api/client'
import { createQueryClient } from '#lib/query-client'

function linkedTradesQuery(fetcher: typeof fetch) {
  return $api.queryOptions('get', '/api/notes/{id}/trades', {
    params: { path: { id: 'fake-note' } },
    fetch: fetcher,
  })
}

function queryClientWithImmediateRetries() {
  const queryClient = createQueryClient()
  queryClient.setDefaultOptions({
    ...queryClient.getDefaultOptions(),
    queries: {
      ...queryClient.getDefaultOptions().queries,
      retryDelay: 0,
    },
  })
  return queryClient
}

function normalizeError(error: unknown): unknown {
  return error instanceof Error
    ? { name: error.name, message: error.message }
    : error
}

function respondWithError(status: number, message: string): typeof fetch {
  return () => Promise.resolve(Response.json({ error: message }, { status }))
}

async function runQuery(fetcher: typeof fetch): Promise<string> {
  let requestCount = 0
  const countedFetcher: typeof fetch = (...args) => {
    requestCount += 1
    return fetcher(...args)
  }
  const result = await queryClientWithImmediateRetries()
    .fetchQuery(linkedTradesQuery(countedFetcher))
    .then(
      () => ({ state: 'success' as const, error: null }),
      (error: unknown) => ({
        state: 'error' as const,
        error: normalizeError(error),
      }),
    )

  return JSON.stringify({ requestCount, result })
}

describe('createQueryClient', () => {
  it.each([
    [
      'does not retry a 400 response',
      respondWithError(400, 'invalid request'),
      JSON.stringify({
        requestCount: 1,
        result: {
          state: 'error',
          error: { error: 'invalid request', status: 400 },
        },
      }),
    ],
    [
      'does not retry a 404 response',
      respondWithError(404, 'not found'),
      JSON.stringify({
        requestCount: 1,
        result: { state: 'error', error: { error: 'not found', status: 404 } },
      }),
    ],
    [
      'retries a 500 response three times',
      respondWithError(500, 'server error'),
      JSON.stringify({
        requestCount: 4,
        result: {
          state: 'error',
          error: { error: 'server error', status: 500 },
        },
      }),
    ],
    [
      'retries a communication error three times',
      () => Promise.reject(new TypeError('connection failed')),
      JSON.stringify({
        requestCount: 4,
        result: {
          state: 'error',
          error: { name: 'TypeError', message: 'connection failed' },
        },
      }),
    ],
  ] as const)('%s', async (_name, fetcher, expected) => {
    expect(await runQuery(fetcher)).toBe(expected)
  })
})
