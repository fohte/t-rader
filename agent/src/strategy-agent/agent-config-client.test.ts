import { ok } from 'neverthrow'
import { afterEach, describe, expect, it, vi } from 'vitest'

import {
  AgentConfigFetchError,
  createAgentConfigFetcher,
} from '@/strategy-agent/agent-config-client'

afterEach(() => {
  vi.unstubAllGlobals()
})

describe('createAgentConfigFetcher', () => {
  it('fetches and maps the backend agent-config response to camelCase', async () => {
    const fetchMock = vi.fn(
      // eslint-disable-next-line @typescript-eslint/no-unused-vars -- typed only so fetchMock.mock.calls[0][0] below is typed, the response body doesn't depend on it
      (_input: string | URL | Request): Promise<Response> =>
        Promise.resolve(
          new Response(
            JSON.stringify({
              agents_md: '# AGENTS',
              skills: { 'ja-stock': 'skill body' },
              model: 'opencode-go/minimax-m3',
              small_model: 'opencode-go/deepseek-v4-flash',
            }),
            { status: 200 },
          ),
        ),
    )
    vi.stubGlobal('fetch', fetchMock)

    const fetchAgentConfig = createAgentConfigFetcher('http://backend')
    const result = await fetchAgentConfig('strategy-1')

    expect(result).toEqual(
      ok({
        agentsMd: '# AGENTS',
        skills: { 'ja-stock': 'skill body' },
        model: 'opencode-go/minimax-m3',
        smallModel: 'opencode-go/deepseek-v4-flash',
      }),
    )
    expect(fetchMock.mock.calls[0]?.[0]).toBe(
      'http://backend/api/strategies/strategy-1/agent-config',
    )
  })

  it('returns an error with the strategy id and status when the backend responds with an error', async () => {
    vi.stubGlobal(
      'fetch',
      vi.fn(() => Promise.resolve(new Response('not found', { status: 404 }))),
    )

    const fetchAgentConfig = createAgentConfigFetcher('http://backend')
    const result = await fetchAgentConfig('missing-strategy')

    expect(result.isErr()).toBe(true)
    expect(result._unsafeUnwrapErr()).toEqual(
      new AgentConfigFetchError(
        'failed to fetch agent config for strategy missing-strategy: 404',
      ),
    )
  })

  it('returns an error when the response body does not match the expected shape', async () => {
    vi.stubGlobal(
      'fetch',
      vi.fn(() =>
        Promise.resolve(
          new Response(JSON.stringify({ agents_md: '# AGENTS' }), {
            status: 200,
          }),
        ),
      ),
    )

    const fetchAgentConfig = createAgentConfigFetcher('http://backend')
    const result = await fetchAgentConfig('strategy-1')

    expect(result.isErr()).toBe(true)
    expect(result._unsafeUnwrapErr().message).toBe(
      'malformed agent-config response for strategy strategy-1: expected agents_md/model/small_model strings and a skills map of strings',
    )
  })

  it('returns an error when skills is an array instead of a record', async () => {
    vi.stubGlobal(
      'fetch',
      vi.fn(() =>
        Promise.resolve(
          new Response(
            JSON.stringify({
              agents_md: '# AGENTS',
              skills: ['ja-stock'],
              model: 'opencode-go/minimax-m3',
              small_model: 'opencode-go/deepseek-v4-flash',
            }),
            { status: 200 },
          ),
        ),
      ),
    )

    const fetchAgentConfig = createAgentConfigFetcher('http://backend')
    const result = await fetchAgentConfig('strategy-1')

    expect(result.isErr()).toBe(true)
    expect(result._unsafeUnwrapErr().message).toBe(
      'malformed agent-config response for strategy strategy-1: expected agents_md/model/small_model strings and a skills map of strings',
    )
  })
})
