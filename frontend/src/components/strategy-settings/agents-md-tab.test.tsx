import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { cleanup, render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import type { Middleware } from 'openapi-fetch'
import type { ReactNode } from 'react'
import { afterEach, describe, expect, it } from 'vitest'

import { AgentsMdTab } from '#components/strategy-settings/agents-md-tab'
import { fetchClient } from '#lib/api/client'

interface Store {
  content: string
}

function installMiddleware(initial: Store) {
  const store: Store = { ...initial }
  const middleware: Middleware = {
    onRequest({ request }) {
      const method = request.method.toUpperCase()
      if (/\/api\/strategies\/[^/]+\/agents-md/.test(request.url)) {
        if (method === 'GET') {
          return new Response(JSON.stringify({ content: store.content }), {
            status: 200,
            headers: { 'content-type': 'application/json' },
          })
        }
        if (method === 'PUT') {
          return request
            .clone()
            .json()
            .then((body: { content: string }) => {
              store.content = body.content
              return new Response(JSON.stringify({ content: body.content }), {
                status: 200,
                headers: { 'content-type': 'application/json' },
              })
            })
        }
      }
      throw new Error(`unmocked: ${method} ${request.url}`)
    },
  }
  fetchClient.use(middleware)
  return {
    store,
    eject: () => {
      fetchClient.eject(middleware)
    },
  }
}

let active: ReturnType<typeof installMiddleware> | null = null

function setup(initial: Store) {
  active?.eject()
  active = installMiddleware(initial)
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  })
  function Wrapper({ children }: { children: ReactNode }) {
    return <QueryClientProvider client={client}>{children}</QueryClientProvider>
  }
  return render(<AgentsMdTab strategyId="strat-1" />, { wrapper: Wrapper })
}

afterEach(() => {
  cleanup()
  active?.eject()
  active = null
})

describe('AgentsMdTab', () => {
  it('GET の content を初期値として描画する', async () => {
    setup({ content: '# 戦略方針\n\n本文。' })
    const textarea = await screen.findByLabelText('source')
    await waitFor(() => {
      expect(textarea).toHaveValue('# 戦略方針\n\n本文。')
    })
  })

  it('編集して保存すると、再 GET 後にエディタが新しい content と同期する', async () => {
    const user = userEvent.setup()
    setup({ content: 'old' })

    const textarea = await screen.findByLabelText('source')
    await waitFor(() => {
      expect(textarea).toHaveValue('old')
    })

    await user.clear(textarea)
    await user.type(textarea, 'updated')

    expect(screen.getByTestId('dirty-indicator')).toBeInTheDocument()
    await user.click(screen.getByRole('button', { name: '保存' }))

    await waitFor(() => {
      expect(screen.queryByTestId('dirty-indicator')).toBeNull()
    })
    expect(screen.getByLabelText('source')).toHaveValue('updated')
    expect(active?.store.content).toBe('updated')
  })
})
