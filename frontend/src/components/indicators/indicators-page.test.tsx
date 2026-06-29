import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
  within,
} from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import type { Middleware } from 'openapi-fetch'
import type { ReactNode } from 'react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { IndicatorsPage } from '@/components/indicators/indicators-page'
import { fetchClient } from '@/lib/api/client'

vi.mock(
  '@monaco-editor/react',
  () => import('@/components/indicators/__mocks__/monaco-editor-react'),
)

interface IndicatorRow {
  indicator_id: string
  name: string
  scope: 'global' | 'strategy'
  strategy_id: string | null
  code: string
  input_schema: Record<string, unknown>
  output_schema: Record<string, unknown>
  description: string | null
}

interface Store {
  global: IndicatorRow[]
  strategy: Map<string, IndicatorRow[]>
  // 直近の preview リクエストを記録 (assertion 用)
  previewRequests: unknown[]
}

async function readIndicatorBody(
  request: Request,
): Promise<Partial<IndicatorRow>> {
  // mock middleware は openapi-fetch 経由の JSON しか受け取らないので妥当性は信頼してよい。
  // eslint-disable-next-line @typescript-eslint/no-unsafe-type-assertion -- テスト用のリクエストパース
  return (await request.clone().json()) as Partial<IndicatorRow>
}

function fixedRow(overrides: Partial<IndicatorRow>): IndicatorRow {
  return {
    indicator_id: 'id-' + (overrides.name ?? 'x'),
    name: 'noname',
    scope: 'global',
    strategy_id: null,
    code: "print('{}')",
    input_schema: { type: 'object' },
    output_schema: { type: 'object' },
    description: null,
    ...overrides,
  }
}

function installMiddleware(initial: Partial<Store>) {
  const store: Store = {
    global: initial.global ?? [],
    strategy: initial.strategy ?? new Map(),
    previewRequests: [],
  }
  const middleware: Middleware = {
    async onRequest({ request }) {
      const method = request.method.toUpperCase()
      const url = new URL(request.url)
      const path = url.pathname

      if (method === 'GET' && path === '/api/indicators') {
        return json(store.global)
      }

      const strategyListMatch = path.match(
        /^\/api\/strategies\/([^/]+)\/indicators$/,
      )
      if (method === 'GET' && strategyListMatch != null) {
        return json(store.strategy.get(strategyListMatch[1] ?? '') ?? [])
      }

      if (method === 'POST' && strategyListMatch != null) {
        const body = await readIndicatorBody(request)
        const sid = strategyListMatch[1] ?? ''
        const row = fixedRow({
          ...body,
          indicator_id: `s-${body.name ?? ''}`,
          scope: 'strategy',
          strategy_id: sid,
        })
        const arr = store.strategy.get(sid) ?? []
        arr.push(row)
        store.strategy.set(sid, arr)
        return json(row, 201)
      }

      if (method === 'POST' && path === '/api/indicators') {
        const body = await readIndicatorBody(request)
        const row = fixedRow({
          ...body,
          indicator_id: `g-${body.name ?? ''}`,
          scope: 'global',
        })
        store.global.push(row)
        return json(row, 201)
      }

      if (method === 'PUT' && path.startsWith('/api/indicators/')) {
        const id = path.replace('/api/indicators/', '')
        const body = await readIndicatorBody(request)
        const idx = store.global.findIndex((i) => i.indicator_id === id)
        const existing = store.global[idx]
        if (idx >= 0 && existing != null) {
          const merged: IndicatorRow = { ...existing, ...body }
          store.global[idx] = merged
          return json(merged)
        }
        return json({ error: 'not found' }, 404)
      }

      if (method === 'POST' && path === '/api/indicators/preview') {
        const body = await request.clone().json()
        store.previewRequests.push(body)
        return json({
          output: { value: 42 },
          stdout: '{"value": 42}\n',
          stderr: '',
          exit_code: 0,
        })
      }

      throw new Error(`unmocked: ${method} ${path}`)
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

function json(body: unknown, status = 200) {
  return new Response(JSON.stringify(body), {
    status,
    headers: { 'content-type': 'application/json' },
  })
}

let active: ReturnType<typeof installMiddleware> | null = null

function setup(
  initial: Partial<Store>,
  ui: ReactNode,
): { client: QueryClient } {
  active?.eject()
  active = installMiddleware(initial)
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  })
  function Wrapper({ children }: { children: ReactNode }) {
    return <QueryClientProvider client={client}>{children}</QueryClientProvider>
  }
  render(ui, { wrapper: Wrapper })
  return { client }
}

afterEach(() => {
  cleanup()
  active?.eject()
  active = null
})

describe('IndicatorsPage', () => {
  it('global scope: グローバル indicator のみを一覧表示する', async () => {
    setup(
      {
        global: [
          fixedRow({ indicator_id: 'g1', name: 'rsi-global' }),
          fixedRow({ indicator_id: 'g2', name: 'macd-global' }),
        ],
        strategy: new Map([
          ['s-1', [fixedRow({ indicator_id: 's1', name: 'only-s1' })]],
        ]),
      },
      <IndicatorsPage scope="global" />,
    )

    const list = await screen.findByTestId('indicator-list')
    expect(within(list).getByText('rsi-global')).toBeInTheDocument()
    expect(within(list).getByText('macd-global')).toBeInTheDocument()
    expect(within(list).queryByText('only-s1')).toBeNull()
  })

  it('strategy scope: 指定戦略の indicator のみを一覧表示する', async () => {
    setup(
      {
        global: [fixedRow({ indicator_id: 'g1', name: 'rsi-global' })],
        strategy: new Map([
          ['s-1', [fixedRow({ indicator_id: 's1', name: 'only-s1' })]],
          ['s-2', [fixedRow({ indicator_id: 's2', name: 'only-s2' })]],
        ]),
      },
      <IndicatorsPage scope="strategy" strategyId="s-1" />,
    )

    const list = await screen.findByTestId('indicator-list')
    expect(within(list).getByText('only-s1')).toBeInTheDocument()
    expect(within(list).queryByText('only-s2')).toBeNull()
    expect(within(list).queryByText('rsi-global')).toBeNull()
  })

  it('プレビューを実行すると POST /api/indicators/preview に code/schemas/args が送られ、結果が描画される', async () => {
    const user = userEvent.setup()
    setup(
      {
        global: [
          fixedRow({
            indicator_id: 'g1',
            name: 'rsi',
            code: "print('{}')",
            input_schema: { type: 'object' },
            output_schema: { type: 'object' },
          }),
        ],
      },
      <IndicatorsPage scope="global" />,
    )

    await screen.findByText('rsi')
    const argsArea = await screen.findByLabelText('preview args')
    fireEvent.change(argsArea, { target: { value: '{"period": 14}' } })

    await user.click(screen.getByRole('button', { name: 'プレビュー実行' }))

    await waitFor(() => {
      expect(screen.getByTestId('preview-result')).toBeInTheDocument()
    })
    const result = screen.getByTestId('preview-result')
    expect({
      exitCode: result.querySelector('[data-testid="preview-exit-code"]')
        ?.textContent,
      output: result.querySelector('[data-testid="preview-output"]')
        ?.textContent,
      stdout: result.querySelector('[data-testid="preview-stdout"]')
        ?.textContent,
    }).toEqual({
      exitCode: '0',
      output: '{\n  "value": 42\n}',
      stdout: '{"value": 42}\n',
    })

    expect(active?.store.previewRequests).toEqual([
      {
        code: "print('{}')",
        input_schema: { type: 'object' },
        output_schema: { type: 'object' },
        args: { period: 14 },
      },
    ])
  })

  it('args が JSON として不正な場合、プレビューを送らずエラー表示する', async () => {
    const user = userEvent.setup()
    setup(
      { global: [fixedRow({ indicator_id: 'g1', name: 'rsi' })] },
      <IndicatorsPage scope="global" />,
    )
    await screen.findByText('rsi')

    const argsArea = await screen.findByLabelText('preview args')
    fireEvent.change(argsArea, { target: { value: '{not-json' } })

    await user.click(screen.getByRole('button', { name: 'プレビュー実行' }))

    await waitFor(() => {
      expect(screen.getByTestId('preview-error')).toBeInTheDocument()
    })
    expect(active?.store.previewRequests).toEqual([])
  })

  it('新規 indicator → 保存 で list が invalidate され新行が選択される', async () => {
    const user = userEvent.setup()
    setup({ global: [] }, <IndicatorsPage scope="global" />)

    await user.click(screen.getByRole('button', { name: /新規 indicator/ }))
    const nameInput = await screen.findByLabelText('name')
    await user.type(nameInput, 'fresh')

    await user.click(screen.getByRole('button', { name: '保存' }))
    await waitFor(() => {
      const list = screen.getByTestId('indicator-list')
      expect(within(list).getByText('fresh')).toBeInTheDocument()
    })
  })
})
