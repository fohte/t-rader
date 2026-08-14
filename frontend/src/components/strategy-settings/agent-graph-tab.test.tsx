import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { cleanup, render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import type { Middleware } from 'openapi-fetch'
import type { ReactNode } from 'react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { AgentGraphTab } from '#components/strategy-settings/agent-graph-tab'
import { fetchClient } from '#lib/api/client'

vi.mock(
  '@monaco-editor/react',
  () => import('#components/indicators/__mocks__/monaco-editor-react'),
)

interface Store {
  content: string
}

function installMiddleware(
  initial: Store,
  putResponse?: { status: number; body: unknown },
) {
  const store: Store = { ...initial }
  const middleware: Middleware = {
    onRequest({ request }) {
      const method = request.method.toUpperCase()
      if (/\/api\/strategies\/[^/]+\/agent-graph/.test(request.url)) {
        if (method === 'GET') {
          return new Response(JSON.stringify({ content: store.content }), {
            status: 200,
            headers: { 'content-type': 'application/json' },
          })
        }
        if (method === 'PUT') {
          if (putResponse != null) {
            return new Response(JSON.stringify(putResponse.body), {
              status: putResponse.status,
              headers: { 'content-type': 'application/json' },
            })
          }
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

function setup(
  initial: Store,
  putResponse?: { status: number; body: unknown },
) {
  active?.eject()
  active = installMiddleware(initial, putResponse)
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  })
  function Wrapper({ children }: { children: ReactNode }) {
    return <QueryClientProvider client={client}>{children}</QueryClientProvider>
  }
  return render(<AgentGraphTab strategyId="strat-1" />, { wrapper: Wrapper })
}

afterEach(() => {
  cleanup()
  active?.eject()
  active = null
})

describe('AgentGraphTab', () => {
  it('GET の content を初期値として描画する', async () => {
    setup({ content: 'phases: []' })
    const editor = await screen.findByLabelText('agent_graph')
    await waitFor(() => {
      expect(editor).toHaveValue('phases: []')
    })
  })

  it('編集して保存すると、再 GET 後にエディタが新しい content と同期する', async () => {
    const user = userEvent.setup()
    setup({ content: 'old' })

    const editor = await screen.findByLabelText('agent_graph')
    await waitFor(() => {
      expect(editor).toHaveValue('old')
    })

    await user.clear(editor)
    await user.type(editor, 'updated')

    expect(screen.getByTestId('dirty-indicator')).toBeInTheDocument()
    await user.click(screen.getByRole('button', { name: '保存' }))

    await waitFor(() => {
      expect(screen.queryByTestId('dirty-indicator')).toBeNull()
    })
    expect(screen.getByLabelText('agent_graph')).toHaveValue('updated')
    expect(active?.store.content).toBe('updated')
  })

  it('保存に失敗すると、サーバーが返したエラーメッセージを表示する', async () => {
    const user = userEvent.setup()
    setup(
      { content: 'old' },
      {
        status: 400,
        body: {
          error:
            'phase "investigate": for_each references field "missing_field" which is not defined in phase "plan"\'s output',
        },
      },
    )

    const editor = await screen.findByLabelText('agent_graph')
    await waitFor(() => {
      expect(editor).toHaveValue('old')
    })

    await user.clear(editor)
    await user.type(editor, 'broken')
    await user.click(screen.getByRole('button', { name: '保存' }))

    await waitFor(() => {
      expect(screen.getByTestId('save-error').textContent).toBe(
        'phase "investigate": for_each references field "missing_field" which is not defined in phase "plan"\'s output',
      )
    })
  })
})
