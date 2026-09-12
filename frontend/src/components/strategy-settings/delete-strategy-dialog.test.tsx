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
import { afterEach, describe, expect, it, vi } from 'vitest'

import { DeleteStrategyDialog } from '#components/strategy-settings/delete-strategy-dialog'
import { fetchClient } from '#lib/api/client'

function installMiddleware(deleteResponse: { status: number; body?: unknown }) {
  const deleteCalls: string[] = []
  const middleware: Middleware = {
    onRequest({ request }) {
      const { url } = request
      const method = request.method.toUpperCase()
      if (method === 'DELETE' && /\/api\/strategies\/([^/]+)$/.test(url)) {
        const match = /\/api\/strategies\/([^/]+)$/.exec(url)
        deleteCalls.push(match?.[1] ?? '')
        return new Response(
          deleteResponse.body != null
            ? JSON.stringify(deleteResponse.body)
            : null,
          { status: deleteResponse.status },
        )
      }
      if (method === 'GET' && /\/api\/strategies(\?|$)/.test(url)) {
        return new Response(JSON.stringify([]), {
          status: 200,
          headers: { 'content-type': 'application/json' },
        })
      }
      throw new Error(`unmocked: ${method} ${url}`)
    },
  }
  fetchClient.use(middleware)
  return {
    deleteCalls,
    eject: () => {
      fetchClient.eject(middleware)
    },
  }
}

let active: ReturnType<typeof installMiddleware> | null = null

function setup({
  deleteResponse = { status: 204 },
  onOpenChange = vi.fn(),
}: {
  deleteResponse?: { status: number; body?: unknown }
  onOpenChange?: (open: boolean) => void
} = {}) {
  active?.eject()
  active = installMiddleware(deleteResponse)
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  })
  const rootRoute = createRootRoute({
    component: () => (
      <QueryClientProvider client={client}>
        <DeleteStrategyDialog
          strategyId="strat-1"
          strategyName="半導体短期スイング"
          open
          onOpenChange={onOpenChange}
        />
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
  return { router, onOpenChange }
}

afterEach(() => {
  cleanup()
  active?.eject()
  active = null
})

describe('DeleteStrategyDialog', () => {
  it('戦略名が一致しないと削除ボタンが disabled になる', async () => {
    const user = userEvent.setup()
    setup()

    const input = await screen.findByLabelText(/確認のため戦略名/)
    await user.type(input, '別の名前')

    expect(screen.getByRole('button', { name: '削除する' })).toBeDisabled()
  })

  it('戦略名が一致すると削除ボタンが有効になり、クリックで DELETE してから戦略一覧へ遷移する', async () => {
    const user = userEvent.setup()
    const { router, onOpenChange } = setup()

    const input = await screen.findByLabelText(/確認のため戦略名/)
    await user.type(input, '半導体短期スイング')
    await waitFor(() => {
      expect(input).toHaveValue('半導体短期スイング')
    })
    await user.click(screen.getByRole('button', { name: '削除する' }))

    await waitFor(() => {
      expect(router.state.location.pathname).toBe('/strategies')
    })
    expect(active?.deleteCalls).toEqual(['strat-1'])
    expect(onOpenChange).toHaveBeenCalledWith(false)
  })

  it('削除に失敗するとエラーメッセージを表示し、遷移しない', async () => {
    const user = userEvent.setup()
    const { router } = setup({
      deleteResponse: { status: 500, body: { error: 'internal error' } },
    })

    const input = await screen.findByLabelText(/確認のため戦略名/)
    await user.type(input, '半導体短期スイング')
    await waitFor(() => {
      expect(input).toHaveValue('半導体短期スイング')
    })
    await user.click(screen.getByRole('button', { name: '削除する' }))

    await waitFor(() => {
      expect(screen.getByText('削除に失敗しました')).toBeInTheDocument()
    })
    expect(router.state.location.pathname).toBe('/')
  })

  it('キャンセルを押すと確認欄をリセットして onOpenChange(false) を呼ぶ', async () => {
    const user = userEvent.setup()
    const { onOpenChange } = setup()

    const input = await screen.findByLabelText(/確認のため戦略名/)
    await user.type(input, '途中まで入力')
    await user.click(screen.getByRole('button', { name: 'キャンセル' }))

    expect(onOpenChange).toHaveBeenCalledWith(false)
  })
})
