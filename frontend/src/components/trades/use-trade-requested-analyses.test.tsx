import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { cleanup, renderHook, waitFor } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import type { TradeRequestedAnalysis } from '#components/trades/trade-requested-analyses'
import type { components } from '#lib/api/schema.gen'

type Strategy = components['schemas']['Strategy']

function readHookOutput(
  taskFilters: Array<{
    strategyId: string
    contextKind: string | null
    contextId: string | null
  }>,
  state: 'loading' | 'error' | 'success',
  analyses: TradeRequestedAnalysis[],
) {
  return {
    filters: [...taskFilters].sort((a, b) =>
      a.strategyId.localeCompare(b.strategyId),
    ),
    state,
    analyses,
  }
}

afterEach(() => {
  cleanup()
  vi.unstubAllGlobals()
})

describe('useTradeRequestedAnalyses', () => {
  it('loads every strategy newest first with trade context and attaches notes', async () => {
    const strategies: Strategy[] = [
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
    ]
    const taskFilters: Array<{
      strategyId: string
      contextKind: string | null
      contextId: string | null
    }> = []

    vi.stubGlobal('fetch', (input: RequestInfo | URL) =>
      Promise.resolve().then(() => {
        const url = new URL(
          input instanceof Request ? input.url : input.toString(),
          'http://localhost',
        )
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

        return Response.json({})
      }),
    )

    const { useTradeRequestedAnalyses } =
      await import('#components/trades/use-trade-requested-analyses')
    const queryClient = new QueryClient({
      defaultOptions: { queries: { retry: false } },
    })
    const { result } = renderHook(
      () =>
        useTradeRequestedAnalyses({
          tradeId: 'fake-trade',
          strategies,
          strategyState: 'success',
          enabled: true,
        }),
      {
        wrapper: ({ children }) => (
          <QueryClientProvider client={queryClient}>
            {children}
          </QueryClientProvider>
        ),
      },
    )

    await waitFor(() => {
      expect(
        readHookOutput(
          taskFilters,
          result.current.state,
          result.current.analyses,
        ),
      ).toEqual({
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
        state: 'success',
        analyses: [
          {
            strategyId: 'fake-strategy-b',
            taskId: 'fake-task-new',
            prompt: '新しい分析の依頼',
            phase: 'running',
            createdAt: '2099-02-02T00:00:00Z',
            notes: [
              {
                id: 'fake-note',
                title: '架空銘柄の調査ノート',
                versionId: 'fake-note-version',
              },
            ],
            notesState: 'success',
          },
          {
            strategyId: 'fake-strategy-a',
            taskId: 'fake-task-old',
            prompt: '古い分析の依頼',
            phase: 'completed',
            createdAt: '2099-02-01T00:00:00Z',
            notes: [],
            notesState: 'success',
          },
        ],
      })
    })
  })
})
