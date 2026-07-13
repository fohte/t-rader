import { afterEach, describe, expect, it, vi } from 'vitest'

import { createAgentConfigFetcher } from '@/strategy-agent/agent-config-client'

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
    const config = await fetchAgentConfig('strategy-1')

    expect(config).toEqual({
      agentsMd: '# AGENTS',
      skills: { 'ja-stock': 'skill body' },
      model: 'opencode-go/minimax-m3',
      smallModel: 'opencode-go/deepseek-v4-flash',
    })
    expect(fetchMock.mock.calls[0]?.[0]).toBe(
      'http://backend/api/strategies/strategy-1/agent-config',
    )
  })

  it('throws with the strategy id and status when the backend responds with an error', async () => {
    vi.stubGlobal(
      'fetch',
      vi.fn(() => Promise.resolve(new Response('not found', { status: 404 }))),
    )

    const fetchAgentConfig = createAgentConfigFetcher('http://backend')

    await expect(fetchAgentConfig('missing-strategy')).rejects.toThrow(
      'failed to fetch agent config for strategy missing-strategy: 404',
    )
  })

  it('throws when the response body does not match the expected shape', async () => {
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

    await expect(fetchAgentConfig('strategy-1')).rejects.toThrow(
      'malformed agent-config response for strategy strategy-1',
    )
  })
})
