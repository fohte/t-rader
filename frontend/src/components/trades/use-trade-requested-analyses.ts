import { useQueries } from '@tanstack/react-query'

import type { TradeRequestedAnalysis } from '#components/trades/trade-requested-analyses'
import { $api } from '#lib/api/client'
import type { components } from '#lib/api/schema.gen'

type Strategy = components['schemas']['Strategy']
type StrategyState = 'loading' | 'error' | 'success'

export function useTradeRequestedAnalyses({
  tradeId,
  strategies,
  strategyState,
  enabled,
}: {
  tradeId: string
  strategies: Strategy[] | undefined
  strategyState: StrategyState
  enabled: boolean
}) {
  const strategyTaskQueries = useQueries({
    queries: (strategies ?? []).map((strategy) => ({
      ...$api.queryOptions('get', '/api/strategies/{id}/tasks', {
        params: {
          path: { id: strategy.id },
          query: { context_kind: 'trade', context_id: tradeId },
        },
      }),
      enabled,
    })),
  })
  const tasks = strategyTaskQueries
    .flatMap((query) => query.data ?? [])
    .sort((a, b) => b.created_at.localeCompare(a.created_at))
  const taskNotesQueries = useQueries({
    queries: tasks.map((task) =>
      $api.queryOptions('get', '/api/strategies/{id}/tasks/{task_id}/notes', {
        params: {
          path: { id: task.strategy_id, task_id: task.task_id },
        },
      }),
    ),
  })
  const analyses: TradeRequestedAnalysis[] = tasks.map((task, index) => {
    const notesQuery = taskNotesQueries[index]
    return {
      strategyId: task.strategy_id,
      taskId: task.task_id,
      prompt: task.prompt,
      phase: task.phase,
      createdAt: task.created_at,
      notes: (notesQuery?.data ?? []).map((note) => ({
        id: note.id,
        title: note.title,
        versionId: note.version_id,
      })),
      notesState:
        notesQuery?.isPending === true
          ? 'loading'
          : notesQuery?.isError === true
            ? 'error'
            : 'success',
    }
  })
  const state: StrategyState =
    strategyState === 'loading' ||
    strategyTaskQueries.some((query) => query.isPending)
      ? 'loading'
      : strategyState === 'error' ||
          strategyTaskQueries.some((query) => query.isError)
        ? 'error'
        : 'success'

  return { analyses, state }
}
