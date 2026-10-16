import { err, ok, type Result } from 'neverthrow'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { createSubagentDefinitionFetcher } from '#strategy-agent/subagent-definition-client'

const definitionResponse = {
  id: 'test-id',
  key: 'researcher',
  description: 'Investigate one question',
  model: 'sample-model',
  reasoning_effort: 'medium',
  prompt: 'Use available sources and report evidence.',
  tools: ['search_web', 'read_page'],
  timeout_minutes: 5,
  max_model_calls: 12,
  tool_limits: { search_web: 7, read_page: 4 },
  output: {
    conclusion: { type: 'string' },
    sources: { type: 'array', items: { type: 'string' } },
  },
  created_at: 'test-created-at',
  updated_at: 'test-updated-at',
}

const errorNameAndMessage = <T>(result: Result<T, Error>) =>
  result.mapErr(({ name, message }) => ({ name, message }))

afterEach(() => {
  vi.unstubAllGlobals()
})

describe('createSubagentDefinitionFetcher', () => {
  it('fetches and maps a definition response', async () => {
    const fetchMock = vi.fn<
      (input: string | URL | Request) => Promise<Response>
    >(() => Promise.resolve(new Response(JSON.stringify(definitionResponse))))
    vi.stubGlobal('fetch', fetchMock)

    const fetchDefinition = createSubagentDefinitionFetcher('http://backend/')
    const result = await fetchDefinition('../researcher')

    const actual = Array.of<unknown>(result, fetchMock.mock.calls[0]?.[0])

    expect(actual).toEqual([
      ok({
        key: 'researcher',
        description: 'Investigate one question',
        model: 'sample-model',
        reasoningEffort: 'medium',
        prompt: 'Use available sources and report evidence.',
        tools: ['search_web', 'read_page'],
        timeoutMinutes: 5,
        maxModelCalls: 12,
        toolLimits: { search_web: 7, read_page: 4 },
        output: {
          conclusion: { type: 'string' },
          sources: { type: 'array', items: { type: 'string' } },
        },
      }),
      'http://backend/api/subagent-definitions/..%2Fresearcher',
    ])
  })

  it.each([
    {
      name: 'returns an error for an unsuccessful response',
      fetch: () => Promise.resolve(new Response('missing', { status: 404 })),
      expected: err({
        name: 'SubagentDefinitionFetchError',
        message: 'failed to fetch subagent definition for key missing-key: 404',
      }),
      key: 'missing-key',
    },
    {
      name: 'returns an error for a malformed response',
      fetch: () =>
        Promise.resolve(new Response(JSON.stringify({ key: 'sample' }))),
      expected: err({
        name: 'SubagentDefinitionFetchError',
        message: 'malformed subagent-definition response for key sample',
      }),
      key: 'sample',
    },
    {
      name: 'returns an error when fetch rejects',
      fetch: () => Promise.reject(new Error('network down')),
      expected: err({
        name: 'SubagentDefinitionFetchError',
        message: 'failed to fetch subagent definition for key sample',
      }),
      key: 'sample',
    },
  ])('$name', async ({ fetch, expected, key }) => {
    vi.stubGlobal('fetch', vi.fn(fetch))

    const result = await createSubagentDefinitionFetcher('http://backend')(key)

    expect(errorNameAndMessage(result)).toEqual(expected)
  })
})
