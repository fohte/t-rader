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

async function expectEditorValue(value: string) {
  const editor = await screen.findByLabelText('agent_graph')
  await waitFor(() => {
    expect(editor).toHaveValue(value)
  })
  return editor
}

afterEach(() => {
  cleanup()
  active?.eject()
  active = null
})

describe('AgentGraphTab', () => {
  it('GET の content を初期値として描画する', async () => {
    // フォームで解釈できない内容にして、常に agent_graph エディタ (YAML ビュー) が
    // 出る状態でテストする (有効なフェーズ YAML はデフォルトでフォームビューになるため)
    setup({ content: 'sample content' })
    await expectEditorValue('sample content')
  })

  it('編集すると dirty-indicator が表示される', async () => {
    const user = userEvent.setup()
    setup({ content: 'old' })

    const editor = await expectEditorValue('old')
    await user.clear(editor)
    await user.type(editor, 'updated')

    expect(screen.getByTestId('dirty-indicator')).toBeInTheDocument()
  })

  it('保存すると再 GET 後にエディタが新しい content と同期し、dirty-indicator が消える', async () => {
    const user = userEvent.setup()
    setup({ content: 'old' })

    const editor = await expectEditorValue('old')
    // clear() で一時的に空文字列を経由すると「フェーズ分割 off」の正当な値として
    // フォームビューに切り替わり YAML エディタが外れてしまうため、末尾に追記する
    await user.type(editor, 'updated')
    await user.click(screen.getByRole('button', { name: '保存' }))

    await waitFor(() => {
      expect(screen.queryByTestId('dirty-indicator')).toBeNull()
    })
    await expectEditorValue('oldupdated')
    expect(active?.store.content).toBe('oldupdated')
  })

  it('保存に失敗すると、サーバーが返したエラーメッセージを表示する', async () => {
    const user = userEvent.setup()
    setup(
      { content: 'old' },
      { status: 400, body: { error: 'invalid config' } },
    )

    const editor = await expectEditorValue('old')
    await user.clear(editor)
    await user.type(editor, 'broken')
    await user.click(screen.getByRole('button', { name: '保存' }))

    await waitFor(() => {
      expect(screen.getByTestId('save-error').textContent).toBe(
        'invalid config',
      )
    })
  })
})
