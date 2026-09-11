import { err, ok } from 'neverthrow'
import { afterEach, describe, expect, it, vi } from 'vitest'

import {
  AgentConfigFetchError,
  createAgentConfigFetcher,
} from '#strategy-agent/agent-config-client'

afterEach(() => {
  vi.unstubAllGlobals()
})

describe('createAgentConfigFetcher', () => {
  it('fetches and maps the backend agent-config response to camelCase', async () => {
    const fetchMock = vi.fn<
      (input: string | URL | Request) => Promise<Response>
    >(() =>
      Promise.resolve(
        new Response(
          JSON.stringify({
            agents_md: '# AGENTS',
            skills: { 'ja-stock': 'skill body' },
            model: 'opencode-go/minimax-m3',
            small_model: 'opencode-go/deepseek-v4-flash',
            agent_graph: 'phases: []',
          }),
          { status: 200 },
        ),
      ),
    )
    vi.stubGlobal('fetch', fetchMock)

    const fetchAgentConfig = createAgentConfigFetcher('http://backend')
    const result = await fetchAgentConfig({ purpose: 'purpose-a' })

    expect(result).toEqual(
      ok({
        agentsMd: '# AGENTS',
        skills: { 'ja-stock': 'skill body' },
        model: 'opencode-go/minimax-m3',
        smallModel: 'opencode-go/deepseek-v4-flash',
        agentGraph: 'phases: []',
      }),
    )
    expect(fetchMock.mock.calls[0]?.[0]).toBe(
      'http://backend/api/agent-configs/purpose-a/agent-config',
    )
  })

  it('percent-encodes a purpose containing path separators so it cannot escape the agent-configs path segment', async () => {
    const fetchMock = vi.fn<
      (input: string | URL | Request) => Promise<Response>
    >(() =>
      Promise.resolve(
        new Response(
          JSON.stringify({
            agents_md: '# AGENTS',
            skills: { 'ja-stock': 'skill body' },
            model: 'opencode-go/minimax-m3',
            small_model: 'opencode-go/deepseek-v4-flash',
            agent_graph: 'phases: []',
          }),
          { status: 200 },
        ),
      ),
    )
    vi.stubGlobal('fetch', fetchMock)

    const fetchAgentConfig = createAgentConfigFetcher('http://backend')
    await fetchAgentConfig({ purpose: '../strategies/other-id' })

    expect(fetchMock.mock.calls[0]?.[0]).toBe(
      'http://backend/api/agent-configs/..%2Fstrategies%2Fother-id/agent-config',
    )
  })

  it('returns an error when fetch itself rejects', async () => {
    vi.stubGlobal(
      'fetch',
      vi.fn(() => Promise.reject(new Error('network down'))),
    )

    const fetchAgentConfig = createAgentConfigFetcher('http://backend')
    const result = await fetchAgentConfig({ purpose: 'purpose-a' })

    expect(result).toEqual(
      err(
        new AgentConfigFetchError(
          'failed to fetch agent config for purpose purpose-a',
        ),
      ),
    )
  })

  it('returns an error with the purpose and status when the backend responds with an error', async () => {
    vi.stubGlobal(
      'fetch',
      vi.fn(() => Promise.resolve(new Response('not found', { status: 404 }))),
    )

    const fetchAgentConfig = createAgentConfigFetcher('http://backend')
    const result = await fetchAgentConfig({ purpose: 'missing-purpose' })

    expect(result).toEqual(
      err(
        new AgentConfigFetchError(
          'failed to fetch agent config for purpose missing-purpose: 404',
        ),
      ),
    )
  })

  it('returns an error when the response body is not valid JSON', async () => {
    vi.stubGlobal(
      'fetch',
      vi.fn(() => Promise.resolve(new Response('not json', { status: 200 }))),
    )

    const fetchAgentConfig = createAgentConfigFetcher('http://backend')
    const result = await fetchAgentConfig({ purpose: 'purpose-a' })

    expect(result).toEqual(
      err(
        new AgentConfigFetchError(
          'failed to parse agent-config response body for purpose purpose-a',
        ),
      ),
    )
  })

  it.each([
    {
      name: 'the response body does not match the expected shape',
      body: { agents_md: '# AGENTS' },
    },
    {
      name: 'skills is an array instead of a record',
      body: {
        agents_md: '# AGENTS',
        skills: ['ja-stock'],
        model: 'opencode-go/minimax-m3',
        small_model: 'opencode-go/deepseek-v4-flash',
        agent_graph: '',
      },
    },
    {
      name: 'agent_graph is missing',
      body: {
        agents_md: '# AGENTS',
        skills: { 'ja-stock': 'skill body' },
        model: 'opencode-go/minimax-m3',
        small_model: 'opencode-go/deepseek-v4-flash',
      },
    },
  ])('returns an error when $name', async ({ body }) => {
    vi.stubGlobal(
      'fetch',
      vi.fn(() =>
        Promise.resolve(new Response(JSON.stringify(body), { status: 200 })),
      ),
    )

    const fetchAgentConfig = createAgentConfigFetcher('http://backend')
    const result = await fetchAgentConfig({ purpose: 'purpose-a' })

    expect(result).toEqual(
      err(
        new AgentConfigFetchError(
          'malformed agent-config response for purpose purpose-a: expected agents_md/model/small_model/agent_graph strings and a skills map of strings',
        ),
      ),
    )
  })
})
