import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import {
  createMemoryHistory,
  createRootRoute,
  createRoute,
  createRouter,
  RouterProvider,
} from '@tanstack/react-router'
import { cleanup, render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import type { Middleware } from 'openapi-fetch'
import { afterEach, describe, expect, it } from 'vitest'

import { GeneralTab } from '#components/strategy-settings/general-tab'
import { fetchClient } from '#lib/api/client'

interface Strategy {
  id: string
  name: string
  description: string | null
}

function installMiddleware(
  initial: Strategy,
  patchResponse?: { status: number; body: unknown },
) {
  const store: Strategy = { ...initial }
  const patchCalls: Array<Record<string, unknown>> = []
  const middleware: Middleware = {
    onRequest({ request }) {
      const { url } = request
      const method = request.method.toUpperCase()
      if (/\/api\/strategies\/[^/]+$/.test(url)) {
        if (method === 'GET') {
          return new Response(JSON.stringify(store), {
            status: 200,
            headers: { 'content-type': 'application/json' },
          })
        }
        if (method === 'PATCH') {
          if (patchResponse != null) {
            return request
              .clone()
              .json()
              .then((body: Record<string, unknown>) => {
                patchCalls.push(body)
                return new Response(JSON.stringify(patchResponse.body), {
                  status: patchResponse.status,
                  headers: { 'content-type': 'application/json' },
                })
              })
          }
          return request
            .clone()
            .json()
            .then((body: Partial<Strategy>) => {
              patchCalls.push(body)
              Object.assign(store, body)
              return new Response(JSON.stringify(store), {
                status: 200,
                headers: { 'content-type': 'application/json' },
              })
            })
        }
      }
      throw new Error(`unmocked: ${method} ${url}`)
    },
  }
  fetchClient.use(middleware)
  return {
    store,
    patchCalls,
    eject: () => {
      fetchClient.eject(middleware)
    },
  }
}

let active: ReturnType<typeof installMiddleware> | null = null

function setup(
  initial: Strategy,
  patchResponse?: { status: number; body: unknown },
) {
  active?.eject()
  active = installMiddleware(initial, patchResponse)
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  })
  const rootRoute = createRootRoute({
    component: () => (
      <QueryClientProvider client={client}>
        <GeneralTab strategyId={initial.id} />
      </QueryClientProvider>
    ),
  })
  const strategyListRoute = createRoute({
    getParentRoute: () => rootRoute,
    path: '/strategies',
    component: () => null,
  })
  const router = createRouter({
    routeTree: rootRoute.addChildren([strategyListRoute]),
    history: createMemoryHistory({ initialEntries: ['/'] }),
  })
  render(<RouterProvider router={router} />)
  return router
}

afterEach(() => {
  cleanup()
  active?.eject()
  active = null
})

describe('GeneralTab', () => {
  it('GET の値が名前・説明の入力欄の初期値に反映される', async () => {
    setup({ id: 'strat-1', name: '半導体短期スイング', description: '狙い' })

    await waitFor(() => {
      expect(screen.getByLabelText('戦略名 *')).toHaveValue(
        '半導体短期スイング',
      )
    })
    expect(screen.getByLabelText('説明')).toHaveValue('狙い')
  })

  it('説明が未設定なら入力欄を空にする', async () => {
    setup({ id: 'strat-1', name: '長期投資', description: null })

    await waitFor(() => {
      expect(screen.getByLabelText('説明')).toHaveValue('')
    })
  })

  it('変更がなければ保存ボタンが disabled になる', async () => {
    setup({ id: 'strat-1', name: '長期投資', description: null })

    await waitFor(() => {
      expect(screen.getByRole('button', { name: '保存' })).toBeDisabled()
    })
  })

  it('名前を空にすると保存ボタンが disabled になる', async () => {
    const user = userEvent.setup()
    setup({ id: 'strat-1', name: '長期投資', description: null })

    const nameInput = await screen.findByLabelText('戦略名 *')
    await waitFor(() => {
      expect(nameInput).toHaveValue('長期投資')
    })
    await user.clear(nameInput)

    expect(screen.getByRole('button', { name: '保存' })).toBeDisabled()
  })

  it('名前を編集して保存すると変更したフィールドだけ PATCH される', async () => {
    const user = userEvent.setup()
    setup({ id: 'strat-1', name: '長期投資', description: null })

    const nameInput = await screen.findByLabelText('戦略名 *')
    await waitFor(() => {
      expect(nameInput).toHaveValue('長期投資')
    })
    await user.clear(nameInput)
    await user.type(nameInput, '中期投資')
    await user.click(screen.getByRole('button', { name: '保存' }))

    await waitFor(() => {
      expect(active?.store.name).toBe('中期投資')
    })
    expect(active?.patchCalls).toEqual([{ name: '中期投資' }])
  })

  it('保存に失敗するとエラーメッセージを表示する', async () => {
    const user = userEvent.setup()
    setup(
      { id: 'strat-1', name: '長期投資', description: null },
      { status: 400, body: { error: 'invalid name' } },
    )

    const nameInput = await screen.findByLabelText('戦略名 *')
    await user.clear(nameInput)
    await user.type(nameInput, '中期投資')
    await user.click(screen.getByRole('button', { name: '保存' }))

    await waitFor(() => {
      expect(screen.getByTestId('save-error').textContent).toBe(
        '保存に失敗しました',
      )
    })
  })

  it('戦略を削除ボタンを押すと確認ダイアログに戦略名が表示される', async () => {
    const user = userEvent.setup()
    setup({ id: 'strat-1', name: '長期投資', description: null })

    await user.click(await screen.findByRole('button', { name: '戦略を削除' }))

    expect(
      screen.getByText('確認のため戦略名「長期投資」を入力してください'),
    ).toBeInTheDocument()
  })
})
