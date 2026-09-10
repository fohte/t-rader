import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { cleanup, render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import type { Middleware } from 'openapi-fetch'
import type { ReactNode } from 'react'
import { afterEach, describe, expect, it } from 'vitest'

import { RiskPolicyTab } from '#components/strategy-settings/risk-policy-tab'
import { fetchClient } from '#lib/api/client'

interface Store {
  maxPositionRatio: number | null
}

function installMiddleware(
  initial: Store,
  putResponse?: { status: number; body: unknown },
) {
  const store: Store = { ...initial }
  const middleware: Middleware = {
    onRequest({ request }) {
      const method = request.method.toUpperCase()
      if (/\/api\/strategies\/[^/]+\/risk-policy/.test(request.url)) {
        if (method === 'GET') {
          return new Response(
            JSON.stringify({ max_position_ratio: store.maxPositionRatio }),
            { status: 200, headers: { 'content-type': 'application/json' } },
          )
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
            .then((body: { max_position_ratio: number | null }) => {
              store.maxPositionRatio = body.max_position_ratio
              return new Response(
                JSON.stringify({ max_position_ratio: body.max_position_ratio }),
                {
                  status: 200,
                  headers: { 'content-type': 'application/json' },
                },
              )
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
  return render(<RiskPolicyTab strategyId="strat-1" />, { wrapper: Wrapper })
}

afterEach(() => {
  cleanup()
  active?.eject()
  active = null
})

describe('RiskPolicyTab', () => {
  it('GET の値が入力欄の初期値に反映される', async () => {
    setup({ maxPositionRatio: 0.15 })

    await waitFor(() => {
      expect(screen.getByLabelText(/1 銘柄あたりの保有時価上限/)).toHaveValue(
        '15',
      )
    })
  })

  it('GET の値が現在値表示に反映される', async () => {
    setup({ maxPositionRatio: 0.15 })

    await waitFor(() => {
      expect(screen.getByTestId('current-value').textContent).toBe(
        '現在の上限: 15%',
      )
    })
  })

  it('未設定なら入力欄を空にする', async () => {
    setup({ maxPositionRatio: null })

    const input = await screen.findByLabelText(/1 銘柄あたりの保有時価上限/)
    await waitFor(() => {
      expect(input).toHaveValue('')
    })
  })

  it('未設定なら現在値表示に上限なしと表示する', async () => {
    setup({ maxPositionRatio: null })

    await waitFor(() => {
      expect(screen.getByTestId('current-value').textContent).toBe(
        '現在の上限: 上限なし',
      )
    })
  })

  it('範囲外の値を保存しようとすると PUT せずクライアント側でエラー表示する', async () => {
    const user = userEvent.setup()
    setup({ maxPositionRatio: null })

    const input = await screen.findByLabelText(/1 銘柄あたりの保有時価上限/)
    await user.type(input, '150')
    await user.click(screen.getByRole('button', { name: '保存' }))

    expect(screen.getByTestId('validation-error').textContent).toBe(
      '0 より大きく 100 以下の値を入力してください',
    )
    expect(active?.store.maxPositionRatio).toBeNull()
  })

  it('編集して保存すると PUT され、再 GET 後に現在値表示が更新される', async () => {
    const user = userEvent.setup()
    setup({ maxPositionRatio: null })

    const input = await screen.findByLabelText(/1 銘柄あたりの保有時価上限/)
    await user.type(input, '20')
    await user.click(screen.getByRole('button', { name: '保存' }))

    await waitFor(() => {
      expect(screen.getByTestId('current-value').textContent).toBe(
        '現在の上限: 20%',
      )
    })
    expect(active?.store.maxPositionRatio).toBe(0.2)
  })

  it('空欄で保存すると max_position_ratio: null を PUT して上限を解除する', async () => {
    const user = userEvent.setup()
    setup({ maxPositionRatio: 0.15 })

    const input = await screen.findByLabelText(/1 銘柄あたりの保有時価上限/)
    await waitFor(() => {
      expect(input).toHaveValue('15')
    })
    await user.clear(input)
    await user.click(screen.getByRole('button', { name: '保存' }))

    await waitFor(() => {
      expect(screen.getByTestId('current-value').textContent).toBe(
        '現在の上限: 上限なし',
      )
    })
    expect(active?.store.maxPositionRatio).toBeNull()
  })

  it('保存に失敗するとエラーメッセージを表示する', async () => {
    const user = userEvent.setup()
    setup(
      { maxPositionRatio: null },
      { status: 400, body: { error: 'invalid max_position_ratio' } },
    )

    const input = await screen.findByLabelText(/1 銘柄あたりの保有時価上限/)
    await user.type(input, '20')
    await user.click(screen.getByRole('button', { name: '保存' }))

    await waitFor(() => {
      expect(screen.getByTestId('save-error').textContent).toBe(
        '保存に失敗しました',
      )
    })
  })
})
