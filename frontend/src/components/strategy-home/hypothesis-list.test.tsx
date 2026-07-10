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

import { HypothesisList } from '@/components/strategy-home/hypothesis-list'
import { fetchClient } from '@/lib/api/client'
import type { components } from '@/lib/api/schema.gen'

type Hypothesis = components['schemas']['Hypothesis']

interface CreateBody {
  title: string
  body: string
}

interface HypothesisStore {
  byStrategy: Map<string, Hypothesis[]>
  createCalls: Array<{ strategyId: string; body: CreateBody }>
}

function makeHypothesis(overrides: Partial<Hypothesis> = {}): Hypothesis {
  return {
    hypothesis_id: overrides.hypothesis_id ?? crypto.randomUUID(),
    strategy_id: overrides.strategy_id ?? 'strat-1',
    title: overrides.title ?? 'title',
    body: overrides.body ?? 'body',
    status: overrides.status ?? 'unverified',
    related_note_ids: overrides.related_note_ids ?? [],
    related_interest_ids: overrides.related_interest_ids ?? [],
    created_at: overrides.created_at ?? '2026-01-01T00:00:00Z',
    updated_at: overrides.updated_at ?? '2026-01-01T00:00:00Z',
  }
}

function installMiddleware(initial: Hypothesis[] = []) {
  const store: HypothesisStore = {
    byStrategy: new Map(),
    createCalls: [],
  }
  for (const h of initial) {
    const list = store.byStrategy.get(h.strategy_id) ?? []
    list.push(h)
    store.byStrategy.set(h.strategy_id, list)
  }

  const middleware: Middleware = {
    async onRequest({ request }) {
      const { url } = request
      const method = request.method.toUpperCase()

      const listMatch = /\/api\/strategies\/([^/]+)\/hypotheses(?:\?|$)/.exec(
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
          const created = makeHypothesis({
            strategy_id: sid,
            title: body.title,
            body: body.body,
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

// Link が親ルートを要求するため、最低限のテストルーターを噛ませる
async function renderInRouter(initial: Hypothesis[] = []) {
  activeMiddleware?.eject()
  activeMiddleware = installMiddleware(initial)
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  })
  const rootRoute = createRootRoute({
    component: () => (
      <QueryClientProvider client={client}>
        <HypothesisList strategyId="strat-1" />
      </QueryClientProvider>
    ),
  })
  const detailRoute = createRoute({
    getParentRoute: () => rootRoute,
    path: '/strategies/$id/hypotheses/$hypothesisId',
    component: () => null,
  })
  const router = createRouter({
    routeTree: rootRoute.addChildren([detailRoute]),
    history: createMemoryHistory({ initialEntries: ['/'] }),
  })
  render(<RouterProvider router={router} />)
  await waitFor(() => {
    expect(
      document.body.firstElementChild?.children.length ?? 0,
    ).toBeGreaterThan(0)
  })
}

afterEach(() => {
  cleanup()
  activeMiddleware?.eject()
  activeMiddleware = null
  vi.restoreAllMocks()
})

async function expectEmptyState() {
  await waitFor(() => {
    expect(screen.getByText('—')).toBeInTheDocument()
  })
}

describe('HypothesisList', () => {
  it('既存の仮説を一覧表示する', async () => {
    await renderInRouter([
      makeHypothesis({ title: 'USD/JPY 押し目買い', status: 'supported' }),
    ])

    await waitFor(() => {
      expect(screen.getByText('USD/JPY 押し目買い')).toBeInTheDocument()
    })
    expect(screen.getByTestId('hypothesis-status-pill')).toHaveTextContent(
      '支持',
    )
  })

  it('仮説が無ければ空状態を表示する', async () => {
    await renderInRouter([])
    await expectEmptyState()
  })

  it('+ 追加 から仮説を作成すると一覧に反映され、API に正しい body が送られる', async () => {
    const user = userEvent.setup()
    await renderInRouter([])

    await expectEmptyState()
    await user.click(screen.getByRole('button', { name: '+ 追加' }))

    await user.type(screen.getByLabelText('title *'), '新しい仮説')
    await user.type(screen.getByLabelText('body (Markdown) *'), '根拠本文')
    await user.click(screen.getByRole('button', { name: '作成' }))

    await waitFor(() => {
      expect(screen.getByText('新しい仮説')).toBeInTheDocument()
    })
    expect(activeMiddleware?.store.createCalls).toEqual([
      {
        strategyId: 'strat-1',
        body: { title: '新しい仮説', body: '根拠本文' },
      },
    ])
  })

  it('title が空だと validation エラーになり作成 API は呼ばれない', async () => {
    const user = userEvent.setup()
    await renderInRouter([])

    await expectEmptyState()
    await user.click(screen.getByRole('button', { name: '+ 追加' }))
    await user.type(screen.getByLabelText('body (Markdown) *'), '根拠本文')
    await user.click(screen.getByRole('button', { name: '作成' }))

    expect(screen.getByTestId('create-hypothesis-error').textContent).toBe(
      'title は必須です',
    )
    expect(activeMiddleware?.store.createCalls).toEqual([])
  })
})
