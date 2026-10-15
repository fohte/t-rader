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

describe('createQueryClient', () => {
  it('does not retry a 404 response', async () => {
    let requestCount = 0
    const fetcher: typeof fetch = () => {
      requestCount += 1
      return Promise.resolve(
        Response.json({ error: 'not found' }, { status: 404 }),
      )
    }

    const result = await queryClientWithImmediateRetries()
      .fetchQuery(linkedTradesQuery(fetcher))
      .then(
        () => ({ state: 'success' as const, error: null }),
        (error: unknown) => ({
          state: 'error' as const,
          error: normalizeError(error),
        }),
      )

    const actual = JSON.stringify({ requestCount, result })
    const expected = JSON.stringify({
      requestCount: 1,
      result: { state: 'error', error: { error: 'not found', status: 404 } },
    })
    expect(actual).toBe(expected)
  })

  it('retries a 500 response three times', async () => {
    let requestCount = 0
    const fetcher: typeof fetch = () => {
      requestCount += 1
      return Promise.resolve(
        Response.json({ error: 'server error' }, { status: 500 }),
      )
    }

    const result = await queryClientWithImmediateRetries()
      .fetchQuery(linkedTradesQuery(fetcher))
      .then(
        () => ({ state: 'success' as const, error: null }),
        (error: unknown) => ({
          state: 'error' as const,
          error: normalizeError(error),
        }),
      )

    const actual = JSON.stringify({ requestCount, result })
    const expected = JSON.stringify({
      requestCount: 4,
      result: { state: 'error', error: { error: 'server error', status: 500 } },
    })
    expect(actual).toBe(expected)
  })

  it('retries a communication error three times', async () => {
    let requestCount = 0
    const error = new TypeError('connection failed')
    const fetcher: typeof fetch = () => {
      requestCount += 1
      return Promise.reject(error)
    }

    const result = await queryClientWithImmediateRetries()
      .fetchQuery(linkedTradesQuery(fetcher))
      .then(
        () => ({ state: 'success' as const, error: null }),
        (queryError: unknown) => ({
          state: 'error' as const,
          error: normalizeError(queryError),
        }),
      )

    const actual = JSON.stringify({ requestCount, result })
    const expected = JSON.stringify({
      requestCount: 4,
      result: {
        state: 'error',
        error: { name: error.name, message: error.message },
      },
    })
    expect(actual).toBe(expected)
  })
})
