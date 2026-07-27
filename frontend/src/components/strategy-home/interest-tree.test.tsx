import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import {
  cleanup,
  render,
  screen,
  waitFor,
  within,
} from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import type { Middleware } from 'openapi-fetch'
import type { ReactNode } from 'react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import { InterestTree } from '#components/strategy-home/interest-tree'
import { fetchClient } from '#lib/api/client'
import type { components } from '#lib/api/schema.gen'

type StrategyInterest = components['schemas']['StrategyInterest']

interface CreateBody {
  ref_kind: string
  ref_id: string
  role?: string | null
  origin?: string | null
}

interface InterestStore {
  byStrategy: Map<string, StrategyInterest[]>
  createCalls: Array<{ strategyId: string; body: CreateBody }>
  deleteCalls: Array<{ strategyId: string; refKind: string; refId: string }>
}

function makeInterest(
  overrides: Partial<StrategyInterest> = {},
): StrategyInterest {
  return {
    strategy_id: overrides.strategy_id ?? 'strat-1',
    ref_kind: overrides.ref_kind ?? 'stock',
    ref_id: overrides.ref_id ?? '7203',
    role: overrides.role ?? 'seed',
    origin: overrides.origin ?? 'human',
    created_at: overrides.created_at ?? '2026-01-01T00:00:00Z',
  }
}

function installMiddleware(initial: StrategyInterest[] = []) {
  const store: InterestStore = {
    byStrategy: new Map(),
    createCalls: [],
    deleteCalls: [],
  }
  for (const i of initial) {
    const list = store.byStrategy.get(i.strategy_id) ?? []
    list.push(i)
    store.byStrategy.set(i.strategy_id, list)
  }

  const middleware: Middleware = {
    async onRequest({ request }) {
      const { url } = request
      const method = request.method.toUpperCase()

      const deleteMatch =
        /\/api\/strategies\/([^/]+)\/interests\/([^/]+)\/([^/?]+)/.exec(url)
      if (deleteMatch != null && method === 'DELETE') {
        const sid = deleteMatch[1] ?? ''
        const refKind = deleteMatch[2] ?? ''
        const refId = deleteMatch[3] ?? ''
        store.deleteCalls.push({ strategyId: sid, refKind, refId })
        const list = store.byStrategy.get(sid) ?? []
        const idx = list.findIndex(
          (i) => i.ref_kind === refKind && i.ref_id === refId,
        )
        if (idx < 0) {
          return new Response(JSON.stringify({ error: 'not found' }), {
            status: 404,
            headers: { 'content-type': 'application/json' },
          })
        }
        list.splice(idx, 1)
        store.byStrategy.set(sid, list)
        return new Response(null, { status: 204 })
      }

      const listMatch = /\/api\/strategies\/([^/]+)\/interests(?:\?|$)/.exec(
        url,
      )
      if (listMatch != null) {
        const sid = listMatch[1] ?? ''
        if (method === 'GET') {
          const list = store.byStrategy.get(sid) ?? []
          return new Response(JSON.stringify(list), {
            status: 200,
            headers: { 'content-type': 'application/json' },
          })
        }
        if (method === 'POST') {
          const body: CreateBody = await request.clone().json()
          store.createCalls.push({ strategyId: sid, body })
          const created = makeInterest({
            strategy_id: sid,
            ref_kind: body.ref_kind,
            ref_id: body.ref_id,
            role: body.role ?? 'seed',
            origin: body.origin ?? 'human',
          })
          const list = store.byStrategy.get(sid) ?? []
          list.push(created)
          store.byStrategy.set(sid, list)
          return new Response(JSON.stringify(created), {
            status: 201,
            headers: { 'content-type': 'application/json' },
          })
        }
      }

      throw new Error(`unmocked request: ${method} ${url}`)
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

let activeMiddleware: ReturnType<typeof installMiddleware> | null = null

function setup(initial: StrategyInterest[] = []) {
  activeMiddleware?.eject()
  activeMiddleware = installMiddleware(initial)
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  })
  function Wrapper({ children }: { children: ReactNode }) {
    return <QueryClientProvider client={client}>{children}</QueryClientProvider>
  }
  return render(<InterestTree strategyId="strat-1" />, { wrapper: Wrapper })
}

afterEach(() => {
  cleanup()
  activeMiddleware?.eject()
  activeMiddleware = null
  vi.restoreAllMocks()
})

beforeEach(() => {
  vi.spyOn(window, 'confirm').mockReturnValue(true)
})

async function expectEmptyState() {
  await waitFor(() => {
    expect(screen.getByText('—')).toBeInTheDocument()
  })
}

describe('InterestTree', () => {
  it('関心を role ごとに seed / derived セクションへ分けて表示する', async () => {
    setup([
      makeInterest({ ref_kind: 'stock', ref_id: '7203', role: 'seed' }),
      makeInterest({
        ref_kind: 'indicator',
        ref_id: 'USDJPY',
        role: 'derived',
      }),
    ])

    const seedRow = await screen.findByTestId('interest-row-stock-7203')
    const derivedRow = await screen.findByTestId(
      'interest-row-indicator-USDJPY',
    )
    // Section の見出し (title) は行を包む <ul> の直前の兄弟要素として描画される
    expect(seedRow.closest('ul')?.previousElementSibling).toHaveTextContent(
      'seed',
    )
    expect(derivedRow.closest('ul')?.previousElementSibling).toHaveTextContent(
      'derived',
    )
  })

  it('各関心の origin を人力/LLM の別で可視化する', async () => {
    setup([
      makeInterest({
        ref_kind: 'stock',
        ref_id: '7203',
        role: 'seed',
        origin: 'human',
      }),
      makeInterest({
        ref_kind: 'indicator',
        ref_id: 'USDJPY',
        role: 'derived',
        origin: 'llm',
      }),
    ])

    const humanRow = await screen.findByTestId('interest-row-stock-7203')
    const llmRow = await screen.findByTestId('interest-row-indicator-USDJPY')
    expect(within(humanRow).getByText('人力')).toBeInTheDocument()
    expect(within(llmRow).getByText('LLM')).toBeInTheDocument()
  })

  it('関心が無ければ空状態を表示する', async () => {
    setup([])
    await expectEmptyState()
  })

  it('関心を追加すると一覧に反映され、API に正しい body が送られる', async () => {
    const user = userEvent.setup()
    setup([])

    await expectEmptyState()
    await user.click(screen.getByRole('button', { name: '+ 追加' }))

    await user.selectOptions(screen.getByLabelText('ref_kind'), 'indicator')
    await user.type(screen.getByLabelText('ref_id'), 'USDJPY')
    await user.selectOptions(screen.getByLabelText('role'), 'derived')
    await user.selectOptions(screen.getByLabelText('origin'), 'llm')
    await user.click(screen.getByRole('button', { name: '追加' }))

    await waitFor(() => {
      expect(
        screen.getByTestId('interest-row-indicator-USDJPY'),
      ).toBeInTheDocument()
    })
    expect(activeMiddleware?.store.createCalls).toEqual([
      {
        strategyId: 'strat-1',
        body: {
          ref_kind: 'indicator',
          ref_id: 'USDJPY',
          role: 'derived',
          origin: 'llm',
        },
      },
    ])
  })

  it('ref_id が空だと validation エラーになり作成 API は呼ばれない', async () => {
    const user = userEvent.setup()
    setup([])

    await expectEmptyState()
    await user.click(screen.getByRole('button', { name: '+ 追加' }))
    await user.click(screen.getByRole('button', { name: '追加' }))

    expect(screen.getByTestId('create-interest-error').textContent).toBe(
      'ref_id は必須です',
    )
    expect(activeMiddleware?.store.createCalls).toEqual([])
  })

  it('削除ボタンを押すと確認の上 DELETE が送られ一覧から消える', async () => {
    const user = userEvent.setup()
    setup([makeInterest({ ref_kind: 'stock', ref_id: '7203', role: 'seed' })])

    await screen.findByTestId('interest-row-stock-7203')
    await user.click(
      screen.getByRole('button', { name: '関心 stock:7203 を削除' }),
    )

    await expectEmptyState()
    expect(activeMiddleware?.store.deleteCalls).toEqual([
      { strategyId: 'strat-1', refKind: 'stock', refId: '7203' },
    ])
  })
})
