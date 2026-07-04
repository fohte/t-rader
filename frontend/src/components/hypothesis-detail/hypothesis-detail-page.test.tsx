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

import { HypothesisDetailPage } from '@/components/hypothesis-detail/hypothesis-detail-page'
import { fetchClient } from '@/lib/api/client'
import type { components } from '@/lib/api/schema.gen'

type Hypothesis = components['schemas']['Hypothesis']

interface UpdateBody {
  title?: string
  body?: string
  status?: string
}

interface HypothesisStore {
  byId: Map<string, Hypothesis>
  updateCalls: Array<{ hypothesisId: string; body: UpdateBody }>
}

function makeHypothesis(overrides: Partial<Hypothesis> = {}): Hypothesis {
  return {
    hypothesis_id: overrides.hypothesis_id ?? 'hyp-1',
    strategy_id: overrides.strategy_id ?? 'strat-1',
    title: overrides.title ?? 'old title',
    body: overrides.body ?? 'old body',
    status: overrides.status ?? 'unverified',
    related_note_ids: overrides.related_note_ids ?? [],
    related_interest_ids: overrides.related_interest_ids ?? [],
    created_at: overrides.created_at ?? '2026-01-01T00:00:00Z',
    updated_at: overrides.updated_at ?? '2026-01-01T00:00:00Z',
  }
}

function installMiddleware(initial: Hypothesis) {
  const store: HypothesisStore = {
    byId: new Map([[initial.hypothesis_id, initial]]),
    updateCalls: [],
  }

  const middleware: Middleware = {
    async onRequest({ request }) {
      const { url } = request
      const method = request.method.toUpperCase()

      const singleMatch =
        /\/api\/strategies\/([^/]+)\/hypotheses\/([^/?]+)/.exec(url)
      if (singleMatch != null) {
        const hid = singleMatch[2] ?? ''
        const current = store.byId.get(hid)
        if (current == null) {
          return new Response(JSON.stringify({ error: 'not found' }), {
            status: 404,
            headers: { 'content-type': 'application/json' },
          })
        }
        if (method === 'GET') {
          return new Response(JSON.stringify(current), {
            status: 200,
            headers: { 'content-type': 'application/json' },
          })
        }
        if (method === 'PATCH') {
          const body: UpdateBody = await request.clone().json()
          store.updateCalls.push({ hypothesisId: hid, body })
          const updated: Hypothesis = {
            ...current,
            title: body.title ?? current.title,
            body: body.body ?? current.body,
            status: body.status ?? current.status,
          }
          store.byId.set(hid, updated)
          return new Response(JSON.stringify(updated), {
            status: 200,
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

// 戻るリンクが親ルートを要求するため、最低限のテストルーターを噛ませる
async function renderInRouter(initial: Hypothesis) {
  activeMiddleware?.eject()
  activeMiddleware = installMiddleware(initial)
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  })
  const rootRoute = createRootRoute({
    component: () => (
      <QueryClientProvider client={client}>
        <HypothesisDetailPage
          strategyId="strat-1"
          hypothesisId={initial.hypothesis_id}
        />
      </QueryClientProvider>
    ),
  })
  const strategyHomeRoute = createRoute({
    getParentRoute: () => rootRoute,
    path: '/strategies/$id',
    component: () => null,
  })
  const router = createRouter({
    routeTree: rootRoute.addChildren([strategyHomeRoute]),
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

describe('HypothesisDetailPage', () => {
  it('title/body を編集して保存すると PATCH が送られる', async () => {
    const user = userEvent.setup()
    await renderInRouter(makeHypothesis({ title: 'old', body: 'old body' }))

    const titleInput = await screen.findByLabelText('title')
    await waitFor(() => {
      expect(titleInput).toHaveValue('old')
    })

    await user.clear(titleInput)
    await user.type(titleInput, 'new title')
    await user.click(screen.getByRole('button', { name: '保存' }))

    await waitFor(() => {
      expect(activeMiddleware?.store.updateCalls).toEqual([
        {
          hypothesisId: 'hyp-1',
          body: { title: 'new title', body: 'old body' },
        },
      ])
    })
  })

  it('未編集の場合、保存ボタンは無効化される', async () => {
    await renderInRouter(makeHypothesis())
    const saveButton = await screen.findByRole('button', { name: '保存' })
    expect(saveButton).toBeDisabled()
  })

  it('status を変更すると PATCH { status } が送られ、pill に反映される', async () => {
    const user = userEvent.setup()
    await renderInRouter(makeHypothesis({ status: 'unverified' }))

    const statusSelect = await screen.findByLabelText('status')
    await waitFor(() => {
      expect(statusSelect).toHaveValue('unverified')
    })

    await user.selectOptions(statusSelect, 'supported')

    await waitFor(() => {
      expect(activeMiddleware?.store.updateCalls).toEqual([
        { hypothesisId: 'hyp-1', body: { status: 'supported' } },
      ])
    })
    await waitFor(() => {
      expect(screen.getByLabelText('status')).toHaveValue('supported')
    })
  })
})
