import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { RouterProvider } from '@tanstack/react-router'
import { cleanup, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { closeFloatingChat } from '#components/strategy-shell/floating-chat-store'
import type { components } from '#lib/api/schema.gen'
import { createStoryRouter } from '#storybook/story-router'

type TradeDetail = components['schemas']['TradeDetailResponse']
type Note = components['schemas']['Note']
type FetchHandler = (
  input: RequestInfo | URL,
  init?: RequestInit,
) => Promise<Response>

let fetchHandler: FetchHandler | null = null

function activeFetch(
  input: RequestInfo | URL,
  init?: RequestInit,
): Promise<Response> {
  return (
    fetchHandler?.(input, init) ??
    Promise.resolve(new Response(null, { status: 500 }))
  )
}

function stubFetch(handler: FetchHandler): void {
  fetchHandler = handler
  vi.stubGlobal('fetch', activeFetch)
}

function readReflectionSubmissionOutput(
  draft: { title: string; body: string },
  postRequests: Array<{ path: string; body: unknown }>,
  linkedNote: { title: string | null; href: string | null },
  linkError: string | null,
) {
  return { draft, postRequests, linkedNote, linkError }
}

function readTradeChatOpenOutput() {
  return {
    contextChip: screen.getByText('取引: 2099-02-10 売り 架空銘柄').textContent,
    chatSeed: screen.getByLabelText<HTMLInputElement>('メッセージ入力').value,
    strategyId: screen.getByLabelText<HTMLSelectElement>('戦略').value,
  }
}

afterEach(() => {
  cleanup()
  closeFloatingChat()
  window.localStorage.clear()
  vi.unstubAllGlobals()
  fetchHandler = null
})

describe('TradeDetailPage', () => {
  it('opens chat with the trade context and strategy', async () => {
    const trade: TradeDetail = {
      id: 'fake-trade-sell',
      strategy_id: 'fake-strategy',
      symbol: 'FIC-01',
      side: 'sell',
      qty: 10,
      price: 1000,
      fee: 10,
      date: '2099-02-10',
      source: 'manual',
      note: null,
      created_at: '2099-02-10T00:00:00Z',
      updated_at: '2099-02-10T00:00:00Z',
      realized_pnl: 2200,
      matched_buys: [{ date: '2099-02-01', qty: 10, price: 780 }],
    }
    stubFetch((input: RequestInfo | URL, init?: RequestInit) => {
      const request =
        input instanceof Request ? input : new Request(input, init)
      const path = new URL(request.url).pathname
      if (path === '/api/trades/fake-trade-sell') {
        return Promise.resolve(Response.json(trade))
      }
      if (path === '/api/trades/fake-trade-sell/notes') {
        return Promise.resolve(Response.json([]))
      }
      if (path === '/api/strategies') {
        return Promise.resolve(
          Response.json([
            {
              id: 'fake-strategy',
              name: '検証用戦略',
              description: null,
              sort_order: 0,
              created_at: '2099-01-01T00:00:00Z',
              updated_at: '2099-01-01T00:00:00Z',
            },
          ]),
        )
      }
      if (path === '/api/refs/stocks') {
        return Promise.resolve(
          Response.json([
            {
              id: 'FIC-01',
              name: '架空銘柄',
              market: null,
              sector_id: null,
              created_at: '2099-01-01T00:00:00Z',
              updated_at: '2099-01-01T00:00:00Z',
            },
          ]),
        )
      }
      if (path === '/api/agent-configs') {
        return Promise.resolve(Response.json([]))
      }
      return Promise.resolve(Response.json({}, { status: 200 }))
    })

    const [{ TradeDetailPage }, { FloatingChat }] = await Promise.all([
      import('#components/trades/trade-detail-page'),
      import('#components/strategy-shell/floating-chat'),
    ])
    const queryClient = new QueryClient({
      defaultOptions: { queries: { retry: false } },
    })
    const router = createStoryRouter(
      () => (
        <>
          <FloatingChat />
          <TradeDetailPage tradeId="fake-trade-sell" />
        </>
      ),
      {
        paths: [
          '/trades',
          '/trades/$tradeId',
          '/stocks/$stockId',
          '/notes/$noteId',
        ],
      },
    )
    const user = userEvent.setup()
    render(<RouterProvider router={router} />, {
      wrapper: ({ children }) => (
        <QueryClientProvider client={queryClient}>
          {children}
        </QueryClientProvider>
      ),
    })

    await screen.findByRole('heading', {
      level: 1,
      name: '架空銘柄FIC-01の売り',
    })
    await user.click(
      screen.getByRole('button', { name: 'エージェントに振り返りを頼む' }),
    )
    await screen.findByText(
      'この取引について、売買時の判断とその後の結果を振り返ってください。',
    )

    expect(readTradeChatOpenOutput()).toEqual({
      contextChip: '取引: 2099-02-10 売り 架空銘柄',
      chatSeed:
        'この取引について、売買時の判断とその後の結果を振り返ってください。',
      strategyId: 'fake-strategy',
    })
  }, 30_000)

  it('creates a reflection note and links it to the trade', async () => {
    const trade: TradeDetail = {
      id: 'fake-trade-sell',
      strategy_id: 'fake-strategy',
      symbol: 'FIC-01',
      side: 'sell',
      qty: 10,
      price: 1000,
      fee: 10,
      date: '2099-02-10',
      source: 'manual',
      note: null,
      created_at: '2099-02-10T00:00:00Z',
      updated_at: '2099-02-10T00:00:00Z',
      realized_pnl: 2200,
      matched_buys: [{ date: '2099-02-01', qty: 10, price: 780 }],
    }
    const note: Note = {
      id: 'fake-note',
      version_id: 'fake-note-version',
      version_no: 1,
      is_current: true,
      title: '架空銘柄を売った振り返り',
      body_md: '架空の内容です。',
      frontmatter_json: {},
      resolved_price_references_json: {},
      graphs_json: [],
      tags: [],
      kind: null,
      status: 'approved',
      trigger: null,
      trigger_label: null,
      created_by_kind: 'human',
      created_at: '2099-02-10T00:00:00Z',
      updated_at: '2099-02-10T00:00:00Z',
      execution_id: null,
    }
    const postRequests: Array<{
      path: string
      body: unknown
    }> = []
    let noteLinked = false
    let linkAttempts = 0

    stubFetch(async (input: RequestInfo | URL, init?: RequestInit) => {
      const request =
        input instanceof Request ? input : new Request(input, init)
      const url = new URL(request.url)
      const path = url.pathname
      if (request.method === 'POST') {
        postRequests.push({ path, body: await request.clone().json() })
      }

      if (path === '/api/trades/fake-trade-sell') {
        return Response.json(trade)
      }
      if (
        path === '/api/trades/fake-trade-sell/notes' &&
        request.method === 'GET'
      ) {
        return Response.json(noteLinked ? [note] : [])
      }
      if (
        /^\/api\/strategies\/[^/]+\/tasks$/.test(path) &&
        request.method === 'GET'
      ) {
        return Response.json([])
      }
      if (/^\/api\/strategies\/[^/]+\/tasks\/[^/]+\/notes$/.test(path)) {
        return Response.json([])
      }
      if (path === '/api/strategies') {
        return Response.json([
          {
            id: 'fake-strategy',
            name: '検証用戦略',
            description: null,
            sort_order: 0,
            created_at: '2099-01-01T00:00:00Z',
            updated_at: '2099-01-01T00:00:00Z',
          },
        ])
      }
      if (path === '/api/agent-configs') return Response.json([])
      if (path === '/api/refs/stocks') {
        return Response.json([
          {
            id: 'FIC-01',
            name: '架空銘柄',
            market: null,
            sector_id: null,
            created_at: '2099-01-01T00:00:00Z',
            updated_at: '2099-01-01T00:00:00Z',
          },
        ])
      }
      if (path === '/api/note-kinds') return Response.json([])
      if (path === '/api/notes' && request.method === 'POST') {
        return Response.json(note, { status: 201 })
      }
      if (
        path === '/api/trades/fake-trade-sell/notes' &&
        request.method === 'POST'
      ) {
        linkAttempts += 1
        if (linkAttempts === 1) {
          noteLinked = true
          return Response.json({}, { status: 201 })
        }
        return Response.json({ message: 'link failed' }, { status: 500 })
      }
      return Response.json({}, { status: 200 })
    })

    const [{ TradeDetailPage }, { FloatingChat }] = await Promise.all([
      import('#components/trades/trade-detail-page'),
      import('#components/strategy-shell/floating-chat'),
    ])
    const queryClient = new QueryClient({
      defaultOptions: { queries: { retry: false } },
    })
    const router = createStoryRouter(
      () => (
        <>
          <FloatingChat />
          <TradeDetailPage tradeId="fake-trade-sell" />
        </>
      ),
      {
        paths: [
          '/trades',
          '/trades/$tradeId',
          '/stocks/$stockId',
          '/notes/$noteId',
        ],
      },
    )
    const user = userEvent.setup()
    render(<RouterProvider router={router} />, {
      wrapper: ({ children }) => (
        <QueryClientProvider client={queryClient}>
          {children}
        </QueryClientProvider>
      ),
    })

    await screen.findByRole('heading', {
      level: 1,
      name: '架空銘柄FIC-01の売り',
    })
    await user.click(screen.getByRole('button', { name: '振り返りを書く' }))
    const titleInput =
      await screen.findByLabelText<HTMLInputElement>('タイトル *')
    const bodyInput =
      await screen.findByLabelText<HTMLTextAreaElement>('本文 (Markdown) *')
    const draft = { title: titleInput.value, body: bodyInput.value }

    await user.click(screen.getByRole('button', { name: '作成' }))
    await screen.findByText(note.title)
    await user.click(screen.getByRole('button', { name: '振り返りを書く' }))
    await screen.findByLabelText('タイトル *')
    await user.click(screen.getByRole('button', { name: '作成' }))
    const linkError = await screen.findByRole('alert')
    const noteTitle = screen.getByText(note.title)
    const noteLink = noteTitle.closest('a')

    expect(
      readReflectionSubmissionOutput(
        draft,
        postRequests,
        {
          title: noteTitle.textContent,
          href: noteLink?.getAttribute('href') ?? null,
        },
        linkError.textContent,
      ),
    ).toEqual({
      draft: {
        title: '架空銘柄を売った振り返り',
        body: [
          '2099-02-10 売 10 株 @¥1,000',
          '実現損益 +¥2,200',
          '',
          '[[stock:FIC-01]]',
        ].join('\n'),
      },
      postRequests: [
        {
          path: '/api/notes',
          body: {
            title: '架空銘柄を売った振り返り',
            body_md: [
              '2099-02-10 売 10 株 @¥1,000',
              '実現損益 +¥2,200',
              '',
              '[[stock:FIC-01]]',
            ].join('\n'),
            kind: null,
          },
        },
        {
          path: '/api/trades/fake-trade-sell/notes',
          body: { note_id: 'fake-note' },
        },
        {
          path: '/api/notes',
          body: {
            title: '架空銘柄を売った振り返り',
            body_md: [
              '2099-02-10 売 10 株 @¥1,000',
              '実現損益 +¥2,200',
              '',
              '[[stock:FIC-01]]',
            ].join('\n'),
            kind: null,
          },
        },
        {
          path: '/api/trades/fake-trade-sell/notes',
          body: { note_id: 'fake-note' },
        },
      ],
      linkedNote: {
        title: '架空銘柄を売った振り返り',
        href: '/notes/fake-note?version_id=fake-note-version',
      },
      linkError:
        'ノートを作成しましたが、この取引への紐付けに失敗しました。既存のノートを紐付ける操作から追加してください。',
    })
  }, 30_000)
})
