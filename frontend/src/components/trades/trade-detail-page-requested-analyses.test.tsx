import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { RouterProvider } from '@tanstack/react-router'
import { cleanup, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import type { components } from '#lib/api/schema.gen'
import { createStoryRouter } from '#storybook/story-router'

type TradeDetail = components['schemas']['TradeDetailResponse']

function readRequestedAnalysesOutput(
  taskFilters: Array<{
    strategyId: string
    contextKind: string | null
    contextId: string | null
  }>,
  section: HTMLElement,
) {
  return {
    filters: [...taskFilters].sort((a, b) =>
      a.strategyId.localeCompare(b.strategyId),
    ),
    rows: Array.from(section.querySelectorAll('li')).map((row) => {
      const runDetail = row.querySelector('a[href^="/strategies/"]')
      return {
        prompt: row.querySelector('p')?.textContent,
        phase: row.querySelector('time')?.previousElementSibling?.textContent,
        date: row.querySelector('time')?.textContent,
        noteLinks: Array.from(row.querySelectorAll('a[href^="/notes/"]')).map(
          (link) => ({
            label: link.textContent,
            href: link.getAttribute('href'),
          }),
        ),
        hasNoNotesMessage: row.textContent.includes('ノートはありません'),
        runDetail:
          runDetail == null
            ? null
            : {
                label: runDetail.textContent,
                href: runDetail.getAttribute('href'),
              },
      }
    }),
  }
}

afterEach(() => {
  cleanup()
  vi.unstubAllGlobals()
})

describe('TradeDetailPage requested analyses', () => {
  it('loads all strategies newest first with trade context and shows their notes', async () => {
    const trade: TradeDetail = {
      id: 'fake-trade',
      strategy_id: 'fake-strategy-a',
      symbol: 'FIC-01',
      side: 'sell',
      qty: 10,
      price: 1000,
      fee: 10,
      date: '2099-02-03',
      source: 'manual',
      note: null,
      created_at: '2099-02-03T00:00:00Z',
      updated_at: '2099-02-03T00:00:00Z',
      realized_pnl: 100,
      matched_buys: [],
    }
    const taskFilters: Array<{
      strategyId: string
      contextKind: string | null
      contextId: string | null
    }> = []

    vi.stubGlobal('fetch', (input: RequestInfo | URL, init?: RequestInit) =>
      Promise.resolve().then(() => {
        const request =
          input instanceof Request ? input : new Request(input, init)
        const url = new URL(request.url)
        const taskNotesMatch = url.pathname.match(
          /^\/api\/strategies\/([^/]+)\/tasks\/([^/]+)\/notes$/,
        )
        if (taskNotesMatch != null) {
          return Response.json(
            taskNotesMatch[2] === 'fake-task-new'
              ? [
                  {
                    id: 'fake-note',
                    version_id: 'fake-note-version',
                    title: '架空銘柄の調査ノート',
                  },
                ]
              : [],
          )
        }

        const tasksMatch = url.pathname.match(
          /^\/api\/strategies\/([^/]+)\/tasks$/,
        )
        if (tasksMatch != null) {
          const strategyId = tasksMatch[1] ?? ''
          taskFilters.push({
            strategyId,
            contextKind: url.searchParams.get('context_kind'),
            contextId: url.searchParams.get('context_id'),
          })
          return Response.json([
            {
              task_id:
                strategyId === 'fake-strategy-a'
                  ? 'fake-task-old'
                  : 'fake-task-new',
              strategy_id: strategyId,
              prompt:
                strategyId === 'fake-strategy-a'
                  ? '古い分析の依頼'
                  : '新しい分析の依頼',
              source: 'chat',
              phase: strategyId === 'fake-strategy-a' ? 'completed' : 'running',
              purpose: null,
              created_at:
                strategyId === 'fake-strategy-a'
                  ? '2099-02-01T00:00:00Z'
                  : '2099-02-02T00:00:00Z',
              updated_at: '2099-02-03T00:00:00Z',
              context_kind: 'trade',
              context_id: 'fake-trade',
            },
          ])
        }

        if (url.pathname === '/api/trades/fake-trade') {
          return Response.json(trade)
        }
        if (url.pathname === '/api/trades/fake-trade/notes') {
          return Response.json([])
        }
        if (url.pathname === '/api/strategies') {
          return Response.json([
            {
              id: 'fake-strategy-a',
              name: '架空戦略 A',
              description: null,
              sort_order: 0,
              created_at: '2099-01-01T00:00:00Z',
              updated_at: '2099-01-01T00:00:00Z',
            },
            {
              id: 'fake-strategy-b',
              name: '架空戦略 B',
              description: null,
              sort_order: 1,
              created_at: '2099-01-01T00:00:00Z',
              updated_at: '2099-01-01T00:00:00Z',
            },
          ])
        }
        if (url.pathname === '/api/refs/stocks') return Response.json([])
        return Response.json({})
      }),
    )

    const { TradeDetailPage } =
      await import('#components/trades/trade-detail-page')
    const queryClient = new QueryClient({
      defaultOptions: { queries: { retry: false } },
    })
    const router = createStoryRouter(
      () => <TradeDetailPage tradeId="fake-trade" />,
      {
        paths: [
          '/trades',
          '/stocks/$stockId',
          '/notes/$noteId',
          '/strategies/$id/runs/$taskId',
        ],
      },
    )
    render(<RouterProvider router={router} />, {
      wrapper: ({ children }) => (
        <QueryClientProvider client={queryClient}>
          {children}
        </QueryClientProvider>
      ),
    })

    await screen.findByRole('link', { name: '架空銘柄の調査ノート' })
    const section = screen.getByRole('region', {
      name: 'この取引について頼んだ分析',
    })
    expect(readRequestedAnalysesOutput(taskFilters, section)).toEqual({
      filters: [
        {
          strategyId: 'fake-strategy-a',
          contextKind: 'trade',
          contextId: 'fake-trade',
        },
        {
          strategyId: 'fake-strategy-b',
          contextKind: 'trade',
          contextId: 'fake-trade',
        },
      ],
      rows: [
        {
          prompt: '新しい分析の依頼',
          phase: 'RUNNING',
          date: '2099-02-02',
          noteLinks: [
            {
              label: '架空銘柄の調査ノート',
              href: '/notes/fake-note?version_id=fake-note-version',
            },
          ],
          hasNoNotesMessage: false,
          runDetail: {
            label: '実行詳細 →',
            href: '/strategies/fake-strategy-b/runs/fake-task-new',
          },
        },
        {
          prompt: '古い分析の依頼',
          phase: 'COMPLETED',
          date: '2099-02-01',
          noteLinks: [],
          hasNoNotesMessage: true,
          runDetail: {
            label: '実行詳細 →',
            href: '/strategies/fake-strategy-a/runs/fake-task-old',
          },
        },
      ],
    })
  })
})
